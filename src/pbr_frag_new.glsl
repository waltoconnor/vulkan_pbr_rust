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

float DistributionGGX(vec3 N, vec3 H, float roughness)
{
    float a = roughness*roughness;
    float a2 = a*a;
    float NdotH = max(dot(N, H), 0.0);
    float NdotH2 = NdotH*NdotH;

    float nom   = a2;
    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return nom / denom;
}

float GeometrySchlickGGX(float NdotV, float roughness)
{
    float r = (roughness + 1.0);
    float k = (r*r) / 8.0;

    float nom   = NdotV;
    float denom = NdotV * (1.0 - k) + k;

    return nom / denom;
}

float GeometrySmith(vec3 N, vec3 V, vec3 L, float roughness)
{
    float NdotV = max(dot(N, V), 0.0);
    float NdotL = max(dot(N, L), 0.0);
    float ggx2 = GeometrySchlickGGX(NdotV, roughness);
    float ggx1 = GeometrySchlickGGX(NdotL, roughness);

    return ggx1 * ggx2;
}

vec3 fresnelSchlick(float cosTheta, vec3 F0)
{
    return F0 + (1.0 - F0) * pow(max(1.0 - cosTheta, 0.0), 5.0);
}

vec3 fresnelSchlickRoughness(float cosTheta, vec3 F0, float roughness)
{
    return F0 + (max(vec3(1.0 - roughness), F0) - F0) * pow(max(1.0 - cosTheta, 0.0), 5.0);
}  

void main(){
    // vec3 alb = texture(albedo_t, uv_in).rgb;
    // vec3 N = normalize(norm_in);
    // vec3 L = normalize(light_dir_in);
    // float diff = max(dot(N, L), 0.0);
    // float spec = 0.0;
    // if(diff > 0.0){
    //     vec3 ref = reflect(-L, N);
    //     vec3 camdir = normalize(eye_pos_in - pos_in);
    //     float spec_a = max(dot(ref, camdir), 0.0);
    //     spec = pow(spec_a, 50);
    // }
    // float s = 0.1 + (0.3 * diff) + (10 * spec);

    // f_color = vec4(s * alb.r, s * alb.g, s * alb.b, 1.0);

    vec3 alb = texture(albedo_t, uv_in).rgb;
    float met = texture(metalness_t, uv_in).r;
    float rgh = texture(roughness_t, uv_in).r;
    vec3 nor = texture(normalmap_t, uv_in).rgb;

    vec3 N = normalize(norm_in);
    vec3 V = normalize(eye_pos_in - pos_in);
    vec3 R = reflect(-V, N);

    f_color = vec4(alb, 1.0);

    
}