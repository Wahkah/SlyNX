// Embeds the renderer's deko3d shader binaries (.dksh, compiled by uam from crates/render/shaders/) into the
// program image. The build passes the shader output directory to the assembler with -Wa,-I,<dir>.
    .section .rodata.slynx_render_shaders, "a"

    .balign 256
    .global slynx_render_mesh_vsh
    .global slynx_render_mesh_vsh_end
slynx_render_mesh_vsh:
    .incbin "slynx_render_mesh_vsh.dksh"
slynx_render_mesh_vsh_end:

    .balign 256
    .global slynx_render_mesh_fsh
    .global slynx_render_mesh_fsh_end
slynx_render_mesh_fsh:
    .incbin "slynx_render_mesh_fsh.dksh"
slynx_render_mesh_fsh_end:

    .balign 256
    .global slynx_render_mesh_alpha_fsh
    .global slynx_render_mesh_alpha_fsh_end
slynx_render_mesh_alpha_fsh:
    .incbin "slynx_render_mesh_alpha_fsh.dksh"
slynx_render_mesh_alpha_fsh_end:
