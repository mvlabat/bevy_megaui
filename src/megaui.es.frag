#version 300 es
precision mediump float;
//precision lowp sampler;

in vec2 v_Uv;
in vec4 v_Color;

out vec4 o_Target;

uniform sampler2D MegaUiTexture_texture;

void main() {
    o_Target = v_Color * texture(MegaUiTexture_texture, v_Uv);
}
