# Build the golden test vector generator against knx-openknx sources.
# Usage: make && ./generate

KNX_SRC := $(HOME)/Source/knx-openknx/src/knx
TPUART_SRC := $(shell find $(HOME)/Source/knx-openknx -path '*/TPUart' -type d 2>/dev/null | head -1)

CXX := c++
CXXFLAGS := -std=c++17 -O2 -Wall \
    -I$(KNX_SRC) -I$(KNX_SRC)/.. \
    -DMASK_VERSION=0x57B0 \
    -DNO_KNX_CONFIG \
    -DKNX_NO_PRINT

# Source files needed for DPT + CEMI (minimal subset)
KNX_SRCS := \
    $(KNX_SRC)/dpt.cpp \
    $(KNX_SRC)/dptconvert.cpp \
    $(KNX_SRC)/knx_value.cpp \
    $(KNX_SRC)/cemi_frame.cpp \
    $(KNX_SRC)/npdu.cpp \
    $(KNX_SRC)/tpdu.cpp \
    $(KNX_SRC)/apdu.cpp \
    $(KNX_SRC)/bits.cpp

.PHONY: all clean

all: generate
	./generate
	cp dpt_vectors.json cemi_vectors.json ../knx-core/tests/fixtures/

generate: generate.cpp $(KNX_SRCS)
	$(CXX) $(CXXFLAGS) -o $@ $^

clean:
	rm -f generate dpt_vectors.json cemi_vectors.json
