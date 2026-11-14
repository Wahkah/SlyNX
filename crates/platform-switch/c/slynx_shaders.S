// Embeds the deko3d shader binaries (.dksh) produced by uam into the program image.
// The build passes the shader output directory to the assembler with -Wa,-I,<dir>.
    .section .rodata.slynx_shaders, "a"

    .balign 256
    .global slynx_color_vsh
    .global slynx_color_vsh_end
slynx_color_vsh:
    .incbin "slynx_color_vsh.dksh"
slynx_color_vsh_end:

    .balign 256
    .global slynx_color_fsh
    .global slynx_color_fsh_end
slynx_color_fsh:
    .incbin "slynx_color_fsh.dksh"
slynx_color_fsh_end:
