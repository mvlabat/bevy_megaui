#version 450

layout(location = 0) in vec2 v_Uv;
layout(location = 1) in vec4 v_Color;

layout(location = 0) out vec4 o_Target;

layout(set = 1, binding = 0) uniform texture2D MegaUiTexture_texture;
layout(set = 1, binding = 1) uniform sampler MegaUiTexture_texture_sampler;

void main() {
    o_Target = v_Color * texture(
        sampler2D(MegaUiTexture_texture, MegaUiTexture_texture_sampler),
        v_Uv);
}
