TARGET=client
LDFLAGS="-L../target/release"
LDLIBS="-lprellblock_client"

# We need to link against some SSL implementation

ifeq ($(OS),Windows_NT)
    $(error Windows is not supported as client for now.)
else
    UNAME_S := $(shell uname -s)
    ifeq ($(UNAME_S),Darwin)
        LDFLAGS += -framework Security -framework CoreFoundation
    endif
endif

all: $(TARGET)

.PHONY: all clean