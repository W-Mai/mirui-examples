############################################################################
# nuttx-demo/Makefile — same Make-based wrapper Slint uses, just renamed
# from EXAMPLES_RUST_SLINT_* to EXAMPLES_RUST_MIRUI_DEMO_*.
############################################################################

include $(APPDIR)/Make.defs

PROGNAME  = $(CONFIG_EXAMPLES_RUST_MIRUI_DEMO_PROGNAME)
PRIORITY  = $(CONFIG_EXAMPLES_RUST_MIRUI_DEMO_PRIORITY)
STACKSIZE = $(CONFIG_EXAMPLES_RUST_MIRUI_DEMO_STACKSIZE)
MODULE    = $(CONFIG_EXAMPLES_RUST_MIRUI_DEMO)

context::
	$(call RUST_CARGO_BUILD,nuttx_demo,$(MIRUI_NUTTX_DEMO_DIR))

clean::
	$(call RUST_CARGO_CLEAN,nuttx_demo,$(MIRUI_NUTTX_DEMO_DIR))

include $(APPDIR)/Application.mk
