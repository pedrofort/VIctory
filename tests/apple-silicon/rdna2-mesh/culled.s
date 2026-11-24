
.text
v_mbcnt_lo_u32_b32 v0, -1, 0
v_mbcnt_hi_u32_b32 v0, -1, v0

v_mov_b32_e32 v1, 0xbf800000
v_mov_b32_e32 v2, 0xbf800000
v_mov_b32_e32 v3, 0x3f000000
v_mov_b32_e32 v4, 0x3f800000
v_mov_b32_e32 v6, 0x40400000

v_cmp_eq_u32_e32 vcc, 2, v0
v_cndmask_b32_e32 v1, v1, v6, vcc

v_cmp_eq_u32_e32 vcc, 1, v0
v_cndmask_b32_e32 v2, v2, v6, vcc

v_mov_b32_e32 v5, 0
v_mov_b32_e32 v7, 0x80200400

v_cmp_eq_u32_e32 vcc, 0, v0
v_cndmask_b32_e32 v5, v5, v7, vcc

exp pos0 v1, v2, v3, v4
exp prim v5, off, off, off done
s_endpgm
