#version 330 core
layout(location=0) in vec2 position;
layout(location=1) in vec2 texcoord;
layout(location=2) in float intensity;
uniform float mesh_aspect;
out vec2 uv;
out float gain;
void main(){gl_Position=vec4(position.x/mesh_aspect,position.y,0.0,1.0);uv=vec2(texcoord.x,1.0-texcoord.y);gain=intensity;}
