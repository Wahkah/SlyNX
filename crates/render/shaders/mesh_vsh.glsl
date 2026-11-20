#version 460
// Mesh vertex shader, shared by both backends. deko3d compiles it with uam as is; the OpenGL backend replaces the
// first line with "#version 430 core". Bindings: uniform block 0 (frame constants); storage blocks 0 (instance
// records, one per instance slot), 1 (bone matrices), 2 (morphed vertices), 3 (per-instance vertex weights) and 4
// (outline edge points).
// The instance slot comes from vertex attribute 4, a per-instance stream of 0, 1, 2, ... offset by the draw's base
// instance, so one multi-draw can cover many meshes and instances. Attributes 5 to 7 are the arena's second stream
// (skinning data, index in the mesh).

layout (location = 0) in vec3 inPos;
layout (location = 1) in vec2 inUv;
layout (location = 2) in vec4 inNormal;  // snorm8 x4, w unused
layout (location = 3) in vec4 inColor;   // unorm8 x4
layout (location = 4) in uint inSlot;
layout (location = 5) in uvec4 inBones;   // u8 x4, integer
layout (location = 6) in vec4 inWeights;  // unorm16 x4
layout (location = 7) in uint inLocal;    // u16, index of the vertex in its mesh

layout (std140, binding = 0) uniform Frame
{
    mat4 viewProj;
    vec4 light;      // xyz: unit vector towards the light, w: ambient
    vec4 fog;        // 1/near, 1/near - 1/far, cap x strength, 1 when on (2: rising past far)
    vec4 fogColor;
    vec4 eye;        // camera position, darkening factor
    vec4 screen;     // target width and height in pixels, outline width in pixels
} frame;

struct Instance
{
    vec4 row0;
    vec4 row1;
    vec4 row2;
    // x: tint RGBA8; y: bits 0..7 texture layer, 8..23 flags (1 Lambert, 2 screen space, 4 skinned, 8 morphed,
    // 16 weights, 32 not darkened, 64 not fogged, 128 flat colour, 256 outline, 512 nearest, 1024 projector
    // receiver, 2048 shadow caster), 24..31 alpha cutoff (unorm8); projector draws: data2.x is the projector's
    // parameter matrix (bone rows: origin and near radius; near and far distance, far radius, strength; margin);
    // z: first morphed vertex; w: first bone matrix. Screen space: data2.xy is the UV scale (float bits).
    // Outline: data2.x is the first edge point.
    uvec4 data;
    // x: first vertex weight; y: bits 0..7 layer of T0, 8..15 layer of T2, 16..23 fog factor, 24 two-colour
    // weights; zw: UV offset. Placed meshes with weights: data.z is the first vertex weight.
    uvec4 data2;
};

layout (std430, binding = 0) readonly buffer Instances
{
    Instance inst[];
};

layout (std430, binding = 1) readonly buffer Bones
{
    vec4 boneRows[];   // three rows per matrix
};

layout (std430, binding = 2) readonly buffer Morph
{
    vec4 morph[];      // position, normal per vertex
};

layout (std430, binding = 3) readonly buffer Weights
{
    uvec2 vweights[];  // (shadow, mid-tone RGB), (highlight RGB, alpha), unorm8 each
};

layout (std430, binding = 4) readonly buffer OutlinePoints
{
    vec4 edgePoints[];  // four per edge: far corner, end, end, far corner; placed meshes: four per piece (three
                        // transform rows, then texture layer, tint bits and grey image layer)
};

layout (location = 0) out vec2 vUv;
layout (location = 1) out vec4 vColor;
layout (location = 2) flat out float vCutoff;
layout (location = 3) flat out uint vFlags;
layout (location = 4) flat out uvec3 vLayers;   // T1 (the texture), T0, T2
layout (location = 5) out vec4 vShadowHigh;     // shadow weight, highlight RGB weights
layout (location = 6) out float vFog;
layout (location = 7) flat out vec4 vFogColor;  // RGB, and the darkening factor for this instance
layout (location = 8) out vec3 vProjPos;          // projector receivers: offset from the projector (x, y), depth below
layout (location = 9) flat out vec4 vProjRange;   // near distance, far distance, near radius, far radius
layout (location = 10) flat out vec2 vProjExtra;  // strength, margin

// Half-width of a projector's square at depth d below it (rendering.md 9.2).
float projRadius(vec4 range, float d)
{
    float span = range.y - range.x;
    if (abs(span) < 1e-6)
        return range.z;
    float t = clamp((d - range.x) / span, 0.0, 1.0);
    return range.z + (range.w - range.z) * t;
}

void main()
{
    Instance i = inst[inSlot];
    uint flags = (i.data.y >> 8) & 0xffffu;

    vec3 pos = inPos;
    vec3 nrm = inNormal.xyz;
    if ((flags & 8u) != 0u) {
        uint k = (i.data.z + inLocal) * 2u;
        pos = morph[k].xyz;
        nrm = morph[k + 1u].xyz;
    }
    if ((flags & 4u) != 0u) {
        // M = sum of w * bone over the non-zero weights; no weights: the vertex stays where it is.
        vec4 r0 = vec4(0.0), r1 = vec4(0.0), r2 = vec4(0.0);
        bool any = false;
        for (int j = 0; j < 4; j++) {
            float w = inWeights[j];
            if (w > 0.0) {
                uint b = (i.data.w + inBones[j]) * 3u;
                r0 += w * boneRows[b];
                r1 += w * boneRows[b + 1u];
                r2 += w * boneRows[b + 2u];
                any = true;
            }
        }
        if (any) {
            vec4 p4 = vec4(pos, 1.0);
            pos = vec3(dot(r0, p4), dot(r1, p4), dot(r2, p4));
            vec3 n = vec3(dot(r0.xyz, nrm), dot(r1.xyz, nrm), dot(r2.xyz, nrm));
            nrm = dot(n, n) > 1e-12 ? normalize(n) : n;
        }
    }

    // Flag 8192: a placed mesh. The vertex's piece (its bone bytes) moves it exactly as an instance transform
    // would, and gives its texture layer and tint.
    // The placement replaces the (identity) instance transform, through the same expression, so the results are
    // bit-identical to drawing the piece as an instance.
    uint layer = i.data.y & 255u;
    uint layerT0 = i.data2.y & 255u;
    vec4 pieceTint = vec4(1.0);
    vec4 m0 = i.row0, m1 = i.row1, m2 = i.row2;
    if ((flags & 8192u) != 0u) {
        uint b = i.data2.x + (inBones.x | (inBones.y << 8u) | (inBones.z << 16u) | (inBones.w << 24u)) * 4u;
        m0 = edgePoints[b];
        m1 = edgePoints[b + 1u];
        m2 = edgePoints[b + 2u];
        vec4 extra = edgePoints[b + 3u];
        layer = uint(extra.x);
        pieceTint = unpackUnorm4x8(floatBitsToUint(extra.y));
        if ((i.data2.y & 0x1000000u) != 0u)
            layerT0 = uint(extra.z);
    }

    vec4 p = vec4(pos, 1.0);
    vec3 world = vec3(dot(m0, p), dot(m1, p), dot(m2, p));
    // Flag 2: screen space (overlay), the transform already maps to clip space.
    gl_Position = (flags & 2u) != 0u ? vec4(world, 1.0) : frame.viewProj * vec4(world, 1.0);
    if ((flags & 256u) != 0u) {
        // Outline edge: corner (vertex index & 3) of the quad of edge (vertex index >> 2). Drawn only when the two
        // far corners lie on the same side of the projected edge (a silhouette), as a line of the outline width.
        uint b = i.data2.x + (inLocal >> 2u) * 4u;
        vec4 c[4];
        bool behind = false;
        for (int k = 0; k < 4; k++) {
            vec4 q = vec4(edgePoints[b + uint(k)].xyz, 1.0);
            c[k] = frame.viewProj * vec4(dot(i.row0, q), dot(i.row1, q), dot(i.row2, q), 1.0);
            behind = behind || c[k].w <= 1e-3;
        }
        vec2 half_size = frame.screen.xy * 0.5;
        vec2 s0 = c[0].xy / c[0].w * half_size, s1 = c[1].xy / c[1].w * half_size;
        vec2 s2 = c[2].xy / c[2].w * half_size, s3 = c[3].xy / c[3].w * half_size;
        vec2 d = s2 - s1;
        float a = d.x * (s0.y - s1.y) - d.y * (s0.x - s1.x);
        float e = d.x * (s3.y - s1.y) - d.y * (s3.x - s1.x);
        uint corner = inLocal & 3u;
        if (behind || a * e < 0.0 || dot(d, d) < 1e-8) {
            gl_Position = vec4(2.0, 2.0, 2.0, 1.0);  // outside the view: the quad collapses
        } else {
            vec4 ep = corner < 2u ? c[1] : c[2];
            vec2 n = normalize(vec2(-d.y, d.x)) * ((corner & 1u) == 0u ? 0.5 : -0.5) * frame.screen.z;
            gl_Position = vec4(ep.xy + n / half_size * ep.w, ep.zw);
        }
    }

    // Projectors (rendering.md 9.2): casters are drawn into the silhouette image, straight down from the projector;
    // receivers pass their offset from it to the fragment shader.
    vProjPos = vec3(0.0);
    vProjRange = vec4(0.0);
    vProjExtra = vec2(0.0);
    if ((flags & 3072u) != 0u) {
        uint pr = i.data2.x * 3u;
        vec4 p0 = boneRows[pr];
        vec4 p1 = boneRows[pr + 1u];
        vec4 range = vec4(p1.x, p1.y, p0.w, p1.z);
        float d = p0.z - world.z;
        if ((flags & 2048u) != 0u) {
            float r = max(abs(projRadius(range, d)), 1e-3);
            gl_Position = vec4((world.x - p0.x) / r, (world.y - p0.y) / r, 0.5, 1.0);
        } else {
            vProjPos = vec3(world.x - p0.x, world.y - p0.y, d);
            vProjRange = range;
            vProjExtra = vec2(p1.w, boneRows[pr + 2u].x);
        }
    }

    // Flag 1: Lambert. Otherwise unlit: texture x vertex colour x tint as they are.
    float shade = 1.0;
    if ((flags & 1u) != 0u) {
        vec3 n = vec3(dot(m0.xyz, nrm), dot(m1.xyz, nrm), dot(m2.xyz, nrm));
        if (dot(n, n) > 1e-6) {
            float ambient = frame.light.w;
            shade = ambient + (1.0 - ambient) * max(dot(normalize(n), frame.light.xyz), 0.0);
        }
    }

    vec4 tint = unpackUnorm4x8(i.data.x) * pieceTint;
    vShadowHigh = vec4(0.0);
    if ((flags & 16u) != 0u) {
        // Weights computed by the caller: the colour carries the mid-tone weights and the alpha.
        // Placed meshes keep their placements in data2.x and their weights base in data.z.
        vec4 a, b;
        if ((i.data2.y & 0x2000000u) != 0u) {
            // Two colours lit here, per frame (Sly 2 rendering.md 2.4, moving objects: four lights, cap 2, twice
            // the scale), from the light block at bone row data.z * 3; outputs on the same byte scale as weights.
            uint R = i.data.z * 3u;
            vec4 W = boneRows[R];
            vec4 mc = boneRows[R + 1u];
            bool onePass = boneRows[R + 2u].x > 0.5;
            vec3 nw = vec3(dot(m0.xyz, nrm), dot(m1.xyz, nrm), dot(m2.xyz, nrm));
            nw = dot(nw, nw) > 1e-12 ? normalize(nw) : nw;
            vec4 v = inColor * 255.0;
            vec3 base = floor((v.rgb * mc.rgb + 255.0) / 512.0);
            float ba = floor((v.a * mc.a + 255.0) / 510.0);
            vec4 L = vec4(0.0, 0.0, 0.0, W.w);
            for (uint k = 0u; k < 4u; k++) {
                vec4 P = boneRows[R + 3u + 3u * k];
                vec4 C = boneRows[R + 4u + 3u * k];
                vec4 Q = boneRows[R + 5u + 3u * k];
                if (Q.w < 0.5)
                    continue;
                vec3 dl = world - P.xyz;
                vec3 l = P.w > 0.5 ? (dot(dl, dl) > 1e-12 ? normalize(dl) : vec3(0.0)) : P.xyz;
                float d = dot(nw, l);
                L += C * (2.0 * max((Q.x + (d + d * d * d) * Q.y) * Q.z, 0.0));
            }
            float M = max(L.r, max(L.g, L.b));
            float r = max(2.0 - M, 0.0);
            vec3 Lc = clamp(L.rgb, 0.0, 2.0);
            float mu = clamp(L.w, 0.0, r);
            float sh = r - mu;
            float beta = (2.0 + 2.0 / 127.0 - sh) * 63.5;
            float I = floor(max(base.r, max(base.g, base.b)) / 255.0 * 127.0);
            vec3 A = clamp(floor((Lc + W.rgb * sh) * I), 0.0, 255.0);
            vec3 B = onePass ? vec3(0.0) : clamp(floor(base * mu * (ba / max(beta, 1e-6))), 0.0, 255.0);
            a = vec4(clamp(ba, 0.0, 255.0), A) / 255.0;
            b = vec4(B, clamp(floor(beta), 0.0, 255.0)) / 255.0;
        } else {
            uvec2 w = vweights[((flags & 8192u) != 0u ? i.data.z : i.data2.x) + inLocal];
            a = unpackUnorm4x8(w.x);
            b = unpackUnorm4x8(w.y);
        }
        vColor = vec4(a.yzw, b.w * tint.a);
        vShadowHigh = vec4(a.x, b.xyz);
    } else {
        vColor = vec4(inColor.rgb * tint.rgb * shade, inColor.a * tint.a);
    }

    vUv = (flags & 2u) != 0u ? inUv * uintBitsToFloat(i.data2.xy) + uintBitsToFloat(i.data2.zw)
                             : inUv + uintBitsToFloat(i.data2.zw);
    vLayers = uvec3(layer, layerT0, (i.data2.y >> 8) & 255u);
    vCutoff = float(i.data.y >> 24) / 255.0;
    // Bit 16: two-colour weights (data2.y bit 24).
    vFlags = flags | (((i.data2.y >> 24) & 1u) << 16);

    // Fog, reciprocal in distance; not for screen space (2) or unfogged (64) instances.
    vFog = 0.0;
    if (frame.fog.w > 0.0 && (flags & 66u) == 0u) {
        float z = max(length(world - frame.eye.xyz), 1e-3);
        // fog.w 2: past the far distance the fog keeps rising (Sly 2 and 3) instead of stopping at the cap.
        float t = max((frame.fog.x - 1.0 / z) / frame.fog.y, 0.0);
        t = frame.fog.w > 1.5 ? t : min(t, 1.0);
        float k = float((i.data2.y >> 16) & 255u) / 255.0;
        vFog = clamp(t * frame.fog.z * k, 0.0, 1.0);
    }
    // Darkening for everything but screen space (2) and undarkened (32) instances.
    vFogColor = vec4(frame.fogColor.rgb, (flags & 34u) != 0u ? 1.0 : frame.eye.w);
}
