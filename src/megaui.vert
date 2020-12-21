#version 450

layout(location = 0) in vec3 Vertex_Position;
layout(location = 1) in vec2 Vertex_Uv;
layout(location = 2) in vec4 Vertex_Color;

layout(location = 0) out vec2 v_Uv;
layout(location = 1) out vec4 v_Color;

layout(set = 0, binding = 0) uniform MegaUiTransform {
    vec2 scale;
    vec2 translation;
};

void main() {
    v_Uv = Vertex_Uv;
    v_Color = Vertex_Color;
    gl_Position = vec4(Vertex_Position * vec3(scale, 1.0) + vec3(translation, 0.0), 1.0);
}
