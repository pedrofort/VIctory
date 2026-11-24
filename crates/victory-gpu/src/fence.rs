//! Fences: a 64-bit value and the events waiting for it. Nothing runs on a
//! GPU yet, so only signals (the title's, later our queues') move one.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::object::{self, FENCE, HRESULT, Iid, Object, S_OK, Table, This, guid};
use crate::resource::PAGEABLE_IIDS;

pub(crate) const FENCE_IIDS: [Iid; 4] = [
    guid(
        0x0a753dcf,
        0xc4d8,
        0x4b91,
        [0xad, 0xf6, 0xbe, 0x5a, 0x60, 0xd9, 0x5a, 0x76],
    ),
    PAGEABLE_IIDS[0],
    PAGEABLE_IIDS[1],
    PAGEABLE_IIDS[2],
];

#[repr(C)]
struct Fence {
    object: Object,
    /// Titles also read it straight from memory, at the fence's GPU
    /// address.
    value: AtomicU64,
    flags: u32,
    /// Events to set once the value reaches theirs (handles as integers).
    waiters: Mutex<Vec<(u64, Waiter)>>,
    changed: Condvar,
}

enum Waiter {
    Event(usize),
    Blocking(Arc<AtomicBool>),
}

fn fence<'a>(this: This) -> &'a Fence {
    // SAFETY: called from fence methods only.
    unsafe { &*this.cast::<Fence>() }
}

pub(crate) extern "system" fn create(
    _: This,
    initial: u64,
    flags: u32,
    iid: *const Iid,
    out: *mut This,
) -> HRESULT {
    tracing::debug!(initial, flags, "fence");
    let fence = Box::new(Fence {
        object: Object::new(&FUNCTIONS),
        value: AtomicU64::new(initial),
        flags,
        waiters: Mutex::new(Vec::new()),
        changed: Condvar::new(),
    });
    object::hand_out(FENCE, fence, iid, &FENCE_IIDS, out)
}

fn set_event(event: usize) {
    #[cfg(windows)]
    // SAFETY: the title passed an event handle.
    unsafe {
        windows_sys::Win32::System::Threading::SetEvent(event as *mut c_void)
    };
    #[cfg(not(windows))]
    let _ = event;
}

extern "system" fn query(this: This, iid: *const Iid, out: *mut This) -> HRESULT {
    object::answer(
        FENCE,
        iid,
        object::is(iid, &FENCE_IIDS).then_some(this),
        out,
    )
}
extern "system" fn release(this: This) -> u32 {
    object::release_boxed::<Fence>(this)
}
extern "system" fn completed(this: This) -> u64 {
    fence(this).value.load(Ordering::Acquire)
}
extern "system" fn set_event_on_completion(this: This, value: u64, event: *mut c_void) -> HRESULT {
    let fence = fence(this);
    let mut waiters = fence.waiters.lock().unwrap_or_else(|e| e.into_inner());
    if fence.value.load(Ordering::Acquire) >= value {
        if !event.is_null() {
            set_event(event as usize);
        }
        return S_OK;
    }
    if event.is_null() {
        // Register while holding the same mutex as Signal.
        // Completion is sticky: a later fence rewind must
        // not erase an already delivered notification.
        let reached = Arc::new(AtomicBool::new(false));

        waiters.push((
            value,
            Waiter::Blocking(reached.clone())
        ));

        while !reached.load(Ordering::Acquire)
            && fence.value.load(Ordering::Acquire) < value
        {
            // Periodic observation also covers changes
            // made directly to the exposed atomic value.
            let result = fence.changed.wait_timeout(
                waiters,
                std::time::Duration::from_millis(10)
            ).unwrap_or_else(|e| e.into_inner());

            waiters = result.0;
        }

        waiters.retain(|(_, waiter)| {
            match waiter {
                Waiter::Blocking(flag) =>
                    !Arc::ptr_eq(flag, &reached),
                Waiter::Event(_) => true,
            }
        });

        return S_OK;
    }

    waiters.push((
        value,
        Waiter::Event(event as usize)
    ));

    S_OK
}
pub(crate) extern "system" fn signal(this: This, value: u64) -> HRESULT {
    let fence = fence(this);
    let mut waiters = fence.waiters.lock().unwrap_or_else(|e| e.into_inner());
    fence.value.store(value, Ordering::Release);
    waiters.retain(|(wanted, waiter)| {
        let reached = *wanted <= value;

        if reached {
            match waiter {
                Waiter::Event(event) => set_event(*event),
                Waiter::Blocking(flag) =>
                    flag.store(true, Ordering::Release),
            }
        }

        !reached
    });

    fence.changed.notify_all();

    S_OK
}

extern "system" fn creation_flags(this: This) -> u32 {
    fence(this).flags
}
/// Where the value lives (GPU addresses are CPU addresses).
extern "system" fn gpu_address(this: This) -> u64 {
    (&raw const fence(this).value) as u64
}

/// IUnknown 3, object 4, device child 1, fence 3, fence 1 (2, a console
/// one among them).
static FUNCTIONS: Table<24> = {
    let mut f = object::d3d12_object(object::missing_table::<FENCE, 24>());
    f[0] = query as *const ();
    f[2] = release as *const ();
    f[7] = object::parent_device as *const (); // 0x38
    f[8] = completed as *const (); // 0x40
    f[9] = set_event_on_completion as *const (); // 0x48
    f[10] = signal as *const (); // 0x50
    f[11] = creation_flags as *const (); // 0x58
    f[12] = gpu_address as *const (); // 0x60
    Table(f)
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::slot;

    #[cfg(windows)]
    #[test]
    fn signals_set_waiting_events() {
        use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
        use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

        let mut f = std::ptr::null();
        assert_eq!(create(std::ptr::null(), 1, 0, &FENCE_IIDS[0], &mut f), S_OK);
        assert_eq!(slot::<extern "system" fn(This) -> u64>(f, 0x40)(f), 1);
        // SAFETY: plain auto-reset event.
        let event = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
        assert!(!event.is_null());
        let wait = slot::<extern "system" fn(This, u64, *mut c_void) -> HRESULT>(f, 0x48);
        let signal = slot::<extern "system" fn(This, u64) -> HRESULT>(f, 0x50);
        assert_eq!(wait(f, 5, event), S_OK);
        // SAFETY: a valid event.
        assert_ne!(unsafe { WaitForSingleObject(event, 0) }, WAIT_OBJECT_0);
        assert_eq!(signal(f, 5), S_OK);
        // SAFETY: as above.
        assert_eq!(unsafe { WaitForSingleObject(event, 0) }, WAIT_OBJECT_0);
        assert_eq!(wait(f, 5, event), S_OK, "already reached");
        // SAFETY: as above.
        assert_eq!(unsafe { WaitForSingleObject(event, 0) }, WAIT_OBJECT_0);
        assert_eq!(wait(f, 5, std::ptr::null_mut()), S_OK);
        // Grand Theft Auto VI reads the value at the fence's address.
        let at = slot::<extern "system" fn(This) -> u64>(f, 0x60)(f);
        // SAFETY: the fence's value.
        assert_eq!(unsafe { *(at as *const u64) }, 5);
        // SAFETY: ours.
        unsafe { CloseHandle(event) };
        assert_eq!(slot::<extern "system" fn(This) -> u32>(f, 0x10)(f), 0);
    }
}


#[cfg(all(test, windows))]
mod systems_fence_conformance {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn blocking_wait_orders_cpu_threads() {
        let mut f: This = std::ptr::null();

        assert_eq!(
            create(
                std::ptr::null(),
                0,
                0,
                &FENCE_IIDS[0],
                &mut f
            ),
            S_OK
        );

        assert_eq!(completed(f), 0);

        // Pass the live pointer as an integer.
        // Its owning reference survives every worker.
        let address = f as usize;

        let mut workers = Vec::new();
        let mut receivers = Vec::new();

        for target in [5u64,5,9,12] {
            let (tx,rx) = mpsc::channel();
            let (ready_tx,ready_rx) = mpsc::channel();

            let worker = std::thread::spawn(move || {
                ready_tx.send(()).unwrap();

                let f = address as This;

                let hr = set_event_on_completion(
                    f,
                    target,
                    std::ptr::null_mut()
                );

                tx.send(hr == S_OK).unwrap();
            });

            ready_rx.recv_timeout(
                Duration::from_secs(2)
            ).unwrap();

            workers.push(worker);
            receivers.push(rx);
        }

        // Before any signal, no thread may complete.
        for (i,rx) in receivers.iter().enumerate() {
            assert!(
                matches!(
                    rx.recv_timeout(
                        Duration::from_millis(100)
                    ),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ),
                "FENCE_PREMATURE_RETURN worker={i}"
            );
        }

        println!("FENCE_INITIAL_BLOCKING=4/4");

        assert_eq!(signal(f,5), S_OK);

        for rx in &receivers[..2] {
            assert_eq!(
                rx.recv_timeout(
                    Duration::from_secs(3)
                ).unwrap(),
                true
            );
        }

        for rx in &receivers[2..] {
            assert!(matches!(
                rx.recv_timeout(
                    Duration::from_millis(100)
                ),
                Err(mpsc::RecvTimeoutError::Timeout)
            ));
        }

        println!("FENCE_PARTIAL_SIGNAL=PASS");

        assert_eq!(signal(f,9), S_OK);

        assert_eq!(
            receivers[2].recv_timeout(
                Duration::from_secs(3)
            ).unwrap(),
            true
        );

        assert!(matches!(
            receivers[3].recv_timeout(
                Duration::from_millis(100)
            ),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        println!("FENCE_ORDERED_RELEASE=PASS");

        assert_eq!(signal(f,12), S_OK);

        assert_eq!(
            receivers[3].recv_timeout(
                Duration::from_secs(3)
            ).unwrap(),
            true
        );

        for worker in workers {
            worker.join().unwrap();
        }

        assert_eq!(completed(f),12);

        assert_eq!(
            set_event_on_completion(
                f,12,std::ptr::null_mut()
            ),
            S_OK
        );

        assert_eq!(
            set_event_on_completion(
                f,3,std::ptr::null_mut()
            ),
            S_OK
        );

        println!("FENCE_ALREADY_REACHED=2/2");

        assert_eq!(release(f),0);

        println!("FENCE_CONCURRENCY_VERIFIED=PASS");
    }
}


#[cfg(all(test, windows))]
mod systems_fence_sticky {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn signal_rewind_and_direct_atomic_update() {
        let mut f: This = std::ptr::null();

        assert_eq!(
            create(
                std::ptr::null(),0,0,
                &FENCE_IIDS[0],&mut f
            ),
            S_OK
        );

        let address = f as usize;

        for (target,direct) in [(10u64,false),(20,true)] {
            let (tx,rx) = mpsc::channel();

            let worker = std::thread::spawn(move || {
                let hr = set_event_on_completion(
                    address as This,
                    target,
                    std::ptr::null_mut()
                );

                tx.send(hr == S_OK).unwrap();
            });

            // Observe actual registration, not just thread start.
            let deadline = Instant::now()
                + Duration::from_secs(3);

            loop {
                let registered = fence(f)
                    .waiters
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .iter()
                    .any(|(wanted,waiter)| {
                        *wanted == target
                        && matches!(waiter,Waiter::Blocking(_))
                    });

                if registered {
                    break;
                }

                assert!(Instant::now()<deadline,
                    "blocking waiter was never registered");

                std::thread::sleep(
                    Duration::from_millis(1)
                );
            }

            if direct {
                // CPU atomic update only: this does not
                // claim to verify GPU memory coherence.
                fence(f).value.store(
                    target,Ordering::Release
                );
            } else {
                assert_eq!(signal(f,target),S_OK);
                assert_eq!(signal(f,0),S_OK);
            }

            assert_eq!(
                rx.recv_timeout(
                    Duration::from_secs(3)
                ).unwrap(),
                true
            );

            worker.join().unwrap();

            assert!(
                fence(f)
                    .waiters
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .is_empty()
            );

            println!(
                "FENCE_{}=PASS",
                if direct {
                    "ATOMIC_OBSERVATION"
                } else {
                    "STICKY_COMPLETION"
                }
            );
        }

        assert_eq!(release(f),0);
    }
}
