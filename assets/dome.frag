#version 330 core
in vec2 uv;
in float gain;
out vec4 color;
uniform sampler2D source_texture;
uniform sampler2D background_texture;
uniform int background_enabled;
uniform int projection,output_mode,stereo_input,eye,blackout,flip_x,flip_y,mesh_projection;
uniform float fisheye_fov,source_fov,perspective_fov,source_aspect,output_aspect,brightness,gamma_value;
uniform vec3 angles;
uniform vec2 offset;
const float PI=3.141592653589793;
vec3 rotate_view(vec3 d){
    float s=sin(angles.z),c=cos(angles.z);d=vec3(c*d.x-s*d.y,s*d.x+c*d.y,d.z);
    s=sin(angles.y);c=cos(angles.y);d=vec3(d.x,c*d.y+s*d.z,-s*d.y+c*d.z);
    s=sin(angles.x);c=cos(angles.x);return vec3(c*d.x+s*d.z,d.y,-s*d.x+c*d.z);
}
vec2 project_source(vec3 d){
    if(projection==4)return vec2(0.5+atan(d.x,d.z)/(2.0*PI),acos(clamp(d.y,-1.0,1.0))/PI);
    if(projection==3)return vec2(0.5+atan(d.x,d.z)/(2.0*PI),0.5-d.y/max(length(d.xz),0.000001)/(2.0*tan(source_fov*0.25)));
    if(projection==2){float r=acos(clamp(d.z,-1.0,1.0))/(source_fov*0.5);if(r>1.0)return vec2(-1.0);return vec2(0.5)+0.5*r*vec2(d.x,-d.y)/max(length(d.xy),0.000001);}
    if(projection==1){if(d.z<=0.0)return vec2(-1.0);return vec2(0.5)+0.5*vec2(d.x,-d.y*source_aspect)/d.z/tan(source_fov*0.5);}
    vec3 a=abs(d);float face,s,t,m;
    if(a.x>=a.y&&a.x>=a.z){m=a.x;t=-d.y;if(d.x>0.0){face=0.0;s=-d.z;}else{face=1.0;s=d.z;}}
    else if(a.y>=a.z){m=a.y;s=d.x;if(d.y>0.0){face=2.0;t=d.z;}else{face=3.0;t=-d.z;}}
    else{m=a.z;t=-d.y;if(d.z>0.0){face=4.0;s=d.x;}else{face=5.0;s=-d.x;}}
    return (vec2(mod(face,3.0),floor(face/3.0))+vec2(0.5)+0.5*vec2(s,t)/m)/vec2(3.0,2.0);
}
vec3 direction_from_mesh(vec2 p){
    if(mesh_projection==4){float lon=(p.x-0.5)*2.0*PI,lat=(0.5-p.y)*PI;return vec3(cos(lat)*sin(lon),sin(lat),cos(lat)*cos(lon));}
    if(mesh_projection==3){float lon=(p.x-0.5)*2.0*PI;return normalize(vec3(sin(lon),(1.0-2.0*p.y)*tan(source_fov*0.25),cos(lon)));}
    if(mesh_projection==1){float t=tan(source_fov*0.5);return normalize(vec3((p.x*2.0-1.0)*t,(1.0-p.y*2.0)*t/source_aspect,1.0));}
    vec2 q=min(p*vec2(3.0,2.0),vec2(2.999999,1.999999));int face=int(floor(q.x))+int(floor(q.y))*3;vec2 st=fract(q)*2.0-1.0;
    if(face==0)return normalize(vec3(1.0,-st.y,-st.x));if(face==1)return normalize(vec3(-1.0,-st.y,st.x));if(face==2)return normalize(vec3(st.x,1.0,st.y));if(face==3)return normalize(vec3(st.x,-1.0,-st.y));if(face==4)return normalize(vec3(st.x,-st.y,1.0));return normalize(vec3(-st.x,-st.y,-1.0));
}
void main(){
    color=vec4(0.0,0.0,0.0,1.0);if(blackout!=0)return;
    vec2 p=uv;float intensity_gain=gain;vec2 backdrop=vec2(-1.0);
    if(output_mode!=3&&projection!=6){
        vec3 d;
        if(output_mode==1&&mesh_projection!=2){d=direction_from_mesh(p);}
        else if(output_mode==2){float t=tan(perspective_fov*0.5);d=normalize(vec3((p.x*2.0-1.0)*t,(1.0-p.y*2.0)*t/output_aspect,1.0));}
        else{
            if(output_mode==0){if(output_aspect>=1.0)p.x=(p.x-0.5)*output_aspect+0.5;else p.y=(p.y-0.5)/output_aspect+0.5;}
            vec2 q=vec2(2.0*p.x-1.0,1.0-2.0*p.y);float r=length(q);if(r>1.0)return;
            float theta=r*fisheye_fov*0.5;d=vec3(q*sin(theta)/max(r,0.000001),cos(theta));
        }
        vec3 o=vec3(offset,0.0);float b=dot(o,d);float dist=-b+sqrt(max(0.0,b*b+1.0-dot(o,o)));d=normalize(o+d*dist);
        d=rotate_view(d);
        float rr=acos(clamp(d.z,-1.0,1.0))/(PI*0.5);
        if(rr<=1.0)backdrop=vec2(0.5)+0.5*rr*vec2(d.x,-d.y)/max(length(d.xy),0.000001);
        p=project_source(d);
    }
    if(p.x<0.0||p.x>1.0||p.y<0.0||p.y>1.0){
        if(background_enabled!=0&&backdrop.x>=0.0&&backdrop.x<=1.0&&backdrop.y>=0.0&&backdrop.y<=1.0){color=vec4(pow(max(texture(background_texture,backdrop).rgb*max(intensity_gain,0.0)*brightness,vec3(0.0)),vec3(1.0/gamma_value)),1.0);}
        return;
    }
    if(flip_x!=0)p.x=1.0-p.x;if(flip_y!=0)p.y=1.0-p.y;
    if(stereo_input==1)p.x=(p.x+float(eye))*0.5;
    if(stereo_input==2)p.y=(p.y+float(eye))*0.5;
    vec3 rgb=texture(source_texture,p).rgb;
    color=vec4(pow(max(rgb*max(intensity_gain,0.0)*brightness,vec3(0.0)),vec3(1.0/gamma_value)),1.0);
}
