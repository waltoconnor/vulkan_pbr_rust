#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;

layout(location = 0) out vec3 pos_out;
layout(location = 1) out vec2 uv_out;
layout(location = 2) out vec3 norm_out;
layout(location = 4) out vec3 eye_pos_out;
layout(location = 5) out vec3 light_dir_out;

// layout(set = 0, binding = 0) uniform Data {
//     mat4 world;
//     mat4 view;
//     mat4 proj;
//     mat4 geo_scale;
//     vec3 eye_pos;
//     vec3 light_pos;
// } uniforms;

layout(set = 0, binding = 0) uniform Data {
    mat4 mvp;
    vec3 camloc;
    vec3 lightdir;
    mat4 rotation;
} uniforms;

void main() {
    // mat4 worldview = uniforms.view * uniforms.world;
    // v_normal = transpose(inverse(mat3(worldview))) * normal;
    // pos = (uniforms.geo_scale * vec4(position, 1.0)).xyz;
    // gl_Position = uniforms.proj * worldview * vec4(position, 1.0);
    // tex_coords = uv;

    // eye_position = uniforms.eye_pos;
    // light_pos = uniforms.light_pos;

    vec4 pos = vec4(position, 1.0);
    gl_Position = uniforms.mvp * pos;

    pos_out = vec3((uniforms.rotation * pos).xyz);
    norm_out = vec3((uniforms.rotation * vec4(normal, 1.0)).xyz);
    eye_pos_out = uniforms.camloc;
    light_dir_out = uniforms.lightdir;
    uv_out = uv;
}