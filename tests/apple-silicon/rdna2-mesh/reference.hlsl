
struct V {
    float4 position : SV_Position;
};

[outputtopology("triangle")]
[numthreads(64,1,1)]
void ms_main(
    uint lane : SV_GroupIndex,
    out vertices V verts[3],
    out indices uint3 tris[1]
) {
    SetMeshOutputCounts(3,1);

    if (lane < 3) {
        float2 p = lane == 0 ? float2(-1,-1) :
                   lane == 1 ? float2(-1, 3) :
                               float2( 3,-1);

        verts[lane].position = float4(p,0.5,1);
    }

    if (lane == 0) {
        tris[0] = uint3(0,1,2);
    }
}

float4 ps_main() : SV_Target0 {
    return float4(0.25,0.5,0.75,1);
}
