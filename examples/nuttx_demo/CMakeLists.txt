# mirui-nuttx-demo CMakeLists.txt — alternative to Make.defs/Makefile
# for boards whose NuttX kbuild uses cmake. `nuttx_add_rust` from
# `apps/cmake/nuttx_add_rust.cmake` is the same RUST_CARGO_BUILD
# invocation, just expressed as a cmake target.

if(CONFIG_EXAMPLES_RUST_MIRUI_DEMO)

  nuttx_add_rust(CRATE_NAME nuttx_demo CRATE_PATH ${CMAKE_CURRENT_SOURCE_DIR})

  nuttx_add_application(
    NAME ${CONFIG_EXAMPLES_RUST_MIRUI_DEMO_PROGNAME}
    STACKSIZE ${CONFIG_EXAMPLES_RUST_MIRUI_DEMO_STACKSIZE}
    PRIORITY ${CONFIG_EXAMPLES_RUST_MIRUI_DEMO_PRIORITY})

endif()
