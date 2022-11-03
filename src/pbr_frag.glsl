#version 450

const float PI = 3.141592;
const float Epsilon = 0.00001;

const vec3 Fdielectric = vec3(0.04);

layout(location = 0) in vec3 pos_in;
layout(location = 1) in vec2 uv_in;
layout(location = 2) in vec3 norm_in;
layout(location = 4) in vec3 eye_pos_in;
layout(location = 5) in vec3 light_dir_in;

layout(set = 0, binding = 1) uniform sampler2D albedo_t;
layout(set = 0, binding = 2) uniform sampler2D roughness_t;
layout(set = 0, binding = 3) uniform sampler2D metalness_t;
layout(set = 0, binding = 4) uniform sampler2D normalmap_t;

layout(location = 0) out vec4 f_color;

mat3 cotangentFrame(vec3 N, vec3 p, vec2 uv) {
  // get edge vectors of the pixel triangle
  vec3 dp1 = dFdx(p);
  vec3 dp2 = dFdy(p);
  vec2 duv1 = dFdx(uv);
  vec2 duv2 = dFdy(uv);

  // solve the linear system
  vec3 dp2perp = cross(dp2, N);
  vec3 dp1perp = cross(N, dp1);
  vec3 T = dp2perp * duv1.x + dp1perp * duv2.x;
  vec3 B = dp2perp * duv1.y + dp1perp * duv2.y;

  // construct a scale-invariant frame 
  float invmax = 1.0 / sqrt(max(dot(T,T), dot(B,B)));
  return mat3(normalize(T * invmax), normalize(B * invmax), N);
}


vec3 perturb(vec3 map, vec3 N, vec3 V, vec2 texcoord) {
  mat3 TBN = cotangentFrame(N, -V, texcoord);
  return normalize(TBN * map);
}

float DistributionGGX(vec3 N, vec3 H, float roughness)
{
    float a = roughness*roughness;
    float a2 = a*a;
    float NdotH = max(dot(N, H), 0.0);
    float NdotH2 = NdotH*NdotH;

    float nom   = a2;
    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return nom / max(denom, 0.0000001); // prevent divide by zero for roughness=0.0 and NdotH=1.0
}
// ----------------------------------------------------------------------------
float GeometrySchlickGGX(float NdotV, float roughness)
{
    float r = (roughness + 1.0);
    float k = (r*r) / 8.0;

    float nom   = NdotV;
    float denom = NdotV * (1.0 - k) + k;

    return nom / denom;
}
// ----------------------------------------------------------------------------
float GeometrySmith(vec3 N, vec3 V, vec3 L, float roughness)
{
    float NdotV = max(dot(N, V), 0.0);
    float NdotL = max(dot(N, L), 0.0);
    float ggx2 = GeometrySchlickGGX(NdotV, roughness);
    float ggx1 = GeometrySchlickGGX(NdotL, roughness);

    return ggx1 * ggx2;
}
// ----------------------------------------------------------------------------
vec3 fresnelSchlick(float cosTheta, vec3 F0)
{
    return F0 + (1.0 - F0) * pow(max(1.0 - cosTheta, 0.0), 5.0);
}

void main() {
    vec3 albedo = texture(albedo_t, uv_in).rgb;
    float metalness = texture(metalness_t, uv_in).r;
    float roughness = texture(roughness_t, uv_in).r;
    vec3 nmap = texture(normalmap_t, uv_in).rgb * 2.0 - 1.0;

    vec3 N = normalize(norm_in);
    vec3 V = normalize(eye_pos_in - pos_in);

    //vec3 N = perturb(nmap, normalize(norm_in), - V, uv_in);

    vec3 F0 = vec3(0.04);

    F0 = mix(F0, albedo, metalness);

    vec3 Lo = vec3(0.0, 0.0, 0.0);

    vec3 L = normalize(light_dir_in);

    vec3 H = normalize(V + L);

    float dist = length(light_dir_in);

    float atten = 1.0 / (dist * dist);

    vec3 radiance = vec3(1.0, 1.0, 1.0) * atten;

    float NDF = DistributionGGX(N, H, roughness);
    float G = GeometrySmith(N, V, L, roughness);
    vec3 F = fresnelSchlick(clamp(dot(H,V), 0.0, 1.0), F0);

    vec3 nom = NDF * G * F;
    float denom = 4 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0);

    vec3 specular = nom / max(denom, 0.001);

    vec3 kS = F;
    vec3 kD = vec3(1.0) - kS;
    kD *= 1.0 - metalness;
    float NdotL = max(dot(N, L), 0.0);

    Lo += (kD * albedo / PI + specular) * radiance * NdotL;
    vec3 ambient = vec3(0.03) * albedo;

    vec3 color = ambient + Lo;

    // HDR tonemapping
    color = color / (color + vec3(1.0));
    // gamma correct
    color = pow(color, vec3(1.0/2.2)); 


    f_color = vec4(color, 1.0);
}

