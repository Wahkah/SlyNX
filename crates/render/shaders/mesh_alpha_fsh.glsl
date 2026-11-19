#version 460
// Mesh fragment shader for the alpha-tested draws: as mesh_fsh.glsl, then texels whose alpha is below the
// instance's cutoff are discarded.

layout (location = 0) in vec2 vUv;
layout (location = 1) in vec4 vColor;
layout (location = 2) flat in float vCutoff;
layout (location = 3) flat in uint vFlags;
layout (location = 4) flat in uvec3 vLayers;
layout (location = 5) in vec4 vShadowHigh;
layout (location = 6) in float vFog;
layout (location = 7) flat in vec4 vFogColor;
layout (location = 8) in vec3 vProjPos;
layout (location = 9) flat in vec4 vProjRange;
layout (location = 10) flat in vec2 vProjExtra;

layout (binding = 0) uniform sampler2DArray pool;
// The projectors' silhouette image (rendering.md 9.2).
layout (binding = 1) uniform sampler2D silhouette;

layout (location = 0) out vec4 outColor;

void main()
{
    vec4 t1 = texture(pool, vec3(vUv, float(vLayers.x)));
    vec4 c;
    if ((vFlags & 65552u) == 65552u) {
        // Two colours (Sly 2, rendering.md 3.2): the grey image (T0) times colour A, plus the coloured image (T1)
        // times colour B, added with alpha texel alpha x B alpha. A and B are on the 0x80 = 1.0 scale (stored / 255).
        const float k = 255.0 / 128.0;
        vec4 g = texture(pool, vec3(vUv, float(vLayers.y)));
        vec3 a = vColor.rgb * k, b = vShadowHigh.yzw * k;
        float aa = vShadowHigh.x * k, ba = vColor.a * k;
        vec3 lit = clamp(g.rgb * a, 0.0, 1.0) + clamp(t1.rgb * b, 0.0, 1.0) * clamp(t1.a * ba, 0.0, 2.0);
        c = vec4(min(lit, vec3(1.0)), g.a * aa);
    } else if ((vFlags & 16u) != 0u) {
        vec3 t0 = texture(pool, vec3(vUv, float(vLayers.y))).rgb;
        vec3 t2 = texture(pool, vec3(vUv, float(vLayers.z))).rgb;
        c = vec4(t0 * vShadowHigh.x + t1.rgb * vColor.rgb + t2 * vShadowHigh.yzw, t1.a * vColor.a);
    } else if ((vFlags & 128u) != 0u) {
        c = vColor;  // flat colour
    } else {
        c = t1 * vColor;
    }
    if ((vFlags & 1024u) != 0u) {
        // Projector receiver: the surface's colour times 1 - m * strength, m the silhouette's alpha at the projected
        // point (0 outside the image). Pixels with alpha 0.9 or less, and points more than the margin above the
        // near plane, are untouched; below the far plane the projection is clamped, without a fade.
        float d = vProjPos.z;
        if (c.a <= 0.9 || d < vProjRange.x - vProjExtra.y)
            discard;
        float span = vProjRange.y - vProjRange.x;
        float t = abs(span) < 1e-6 ? 0.0 : clamp((d - vProjRange.x) / span, 0.0, 1.0);
        float r = max(abs(vProjRange.z + (vProjRange.w - vProjRange.z) * t), 1e-3);
        vec2 st = vProjPos.xy / r * 0.5 + 0.5;
        float m = (st.x >= 0.0 && st.x <= 1.0 && st.y >= 0.0 && st.y <= 1.0) ? texture(silhouette, st).a : 0.0;
        float f = 1.0 - m * vProjExtra.x;
        outColor = vec4(f, f, f, 1.0);
        return;
    }
    if (c.a < vCutoff)
        discard;
    c.rgb *= vFogColor.a;
    c.rgb = mix(c.rgb, vFogColor.rgb, vFog);
    outColor = c;
}
