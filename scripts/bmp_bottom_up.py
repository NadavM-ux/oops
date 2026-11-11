"""NSIS wants classic bottom-up 24-bit BMPs; macOS `sips` writes top-down ones. Flip in place."""
import struct, sys

for path in sys.argv[1:]:
    data = bytearray(open(path, "rb").read())
    offset = struct.unpack_from("<I", data, 10)[0]
    width, height = struct.unpack_from("<ii", data, 18)
    bpp = struct.unpack_from("<H", data, 28)[0]
    assert bpp == 24, f"{path}: expected 24-bit, got {bpp}"
    if height < 0:
        h = -height
        stride = (width * 3 + 3) & ~3
        rows = [bytes(data[offset + r * stride: offset + (r + 1) * stride]) for r in range(h)]
        data[offset: offset + h * stride] = b"".join(reversed(rows))
        struct.pack_into("<i", data, 22, h)
        open(path, "wb").write(data)
        print(f"{path}: flipped to bottom-up {width}x{h}")
    else:
        print(f"{path}: already bottom-up")
