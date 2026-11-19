#version 460
// Mesh fragment shader for every draw but the alpha-tested ones (no discard, so early depth testing stays on).
// Texture binding 0: the batch's texture pool, a 2D array; the layers come from the instance record.

layout (location = 0) in vec2 vUv;
layout (location = 1) in vec4 vColor;
layout (location = 2) flat in float vCutoff;
layout (location = 3) flat in uint vFlags;
layout (location = 4) flat in uvec3 vLayers;
layout (location = 5) in vec4 vShadowHigh;
layout (location = 6) in float vFog;
layout (location = 7) flat in vec4 vFogColor;

layout (binding = 0) uniform sampler2DArray pool;
// The movie picture's planes (Y, Cb, Cr), for instances with flag 4096.
layout (binding = 2) uniform sampler2D videoY;
layout (binding = 3) uniform sampler2D videoU;
layout (binding = 4) uniform sampler2D videoV;
// The picture panels' images, for instances with flags 16384 and 32768.
layout (binding = 5) uniform sampler2D panel0;
layout (binding = 6) uniform sampler2D panel1;

layout (location = 0) out vec4 outColor;

void main()
{
    vec2 uv = vUv;
    if ((vFlags & 512u) != 0u) {
        // Pixel text: sample texel centres, so magnified glyphs stay sharp under linear filtering.
        vec2 size = vec2(textureSize(pool, 0).xy);
        uv = (floor(uv * size) + 0.5) / size;
    }
    vec4 t1 = texture(pool, vec3(uv, float(vLayers.x)));
    vec4 c;
    if ((vFlags & 49152u) != 0u) {
        c = ((vFlags & 16384u) != 0u ? texture(panel0, uv) : texture(panel1, uv)) * vColor;
    } else if ((vFlags & 4096u) != 0u) {
        // Movie picture: BT.601, video range.
        float y = (texture(videoY, uv).r - 16.0 / 255.0) * 1.164383;
        float u = texture(videoU, uv).r - 128.0 / 255.0;
        float v = texture(videoV, uv).r - 128.0 / 255.0;
        vec3 rgb = vec3(y + 1.596027 * v, y - 0.391762 * u - 0.812968 * v, y + 2.017232 * u);
        c = vec4(clamp(rgb, 0.0, 1.0), 1.0) * vColor;
    } else if ((vFlags & 65552u) == 65552u) {
        // Two colours (Sly 2, rendering.md 3.2): the grey image (T0) times colour A, plus the coloured image (T1)
        // times colour B, added with alpha texel alpha x B alpha. A and B are on the 0x80 = 1.0 scale (stored / 255).
        const float k = 255.0 / 128.0;
        vec4 g = texture(pool, vec3(vUv, float(vLayers.y)));
        vec3 a = vColor.rgb * k, b = vShadowHigh.yzw * k;
        float aa = vShadowHigh.x * k, ba = vColor.a * k;
        vec3 lit = clamp(g.rgb * a, 0.0, 1.0) + clamp(t1.rgb * b, 0.0, 1.0) * clamp(t1.a * ba, 0.0, 2.0);
        c = vec4(min(lit, vec3(1.0)), g.a * aa);
    } else if ((vFlags & 16u) != 0u) {
        // Three images: T0 x shadow weight + T1 x mid-tone weights + T2 x highlight weights.
        vec3 t0 = texture(pool, vec3(uv, float(vLayers.y))).rgb;
        vec3 t2 = texture(pool, vec3(uv, float(vLayers.z))).rgb;
        c = vec4(t0 * vShadowHigh.x + t1.rgb * vColor.rgb + t2 * vShadowHigh.yzw, t1.a * vColor.a);
    } else if ((vFlags & 128u) != 0u) {
        c = vColor;  // flat colour
    } else {
        c = t1 * vColor;
    }
    c.rgb *= vFogColor.a;
    c.rgb = mix(c.rgb, vFogColor.rgb, vFog);
    outColor = c;
}
