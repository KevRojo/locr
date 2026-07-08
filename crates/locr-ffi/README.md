# locr-ffi — the C ABI that makes locr universal

One frozen `extern "C"` surface (`include/locr.h`) + prebuilt `cdylib`/`staticlib`
per platform = every language on earth can run local OCR with zero cloud calls.

## The whole API

```c
const char *locr_version(void);
LocrStatus  locr_image_to_text(const uint8_t *bytes, size_t len, char **out_text);
void        locr_free_text(char *text);
```

## Consume it from anywhere

### C# / .NET (P/Invoke)

```csharp
[DllImport("locr")] static extern int locr_image_to_text(byte[] bytes, nuint len, out IntPtr text);
[DllImport("locr")] static extern void locr_free_text(IntPtr text);

var bytes = File.ReadAllBytes("invoice.png");
if (locr_image_to_text(bytes, (nuint)bytes.Length, out var p) == 0) {
    Console.WriteLine(Marshal.PtrToStringUTF8(p));
    locr_free_text(p);
}
```

### Java (Panama / FFM)

```java
Linker linker = Linker.nativeLinker();
SymbolLookup locr = SymbolLookup.libraryLookup("locr", Arena.global());
// bind locr_image_to_text / locr_free_text with MethodHandles
```

### Go (cgo)

```go
// #cgo LDFLAGS: -llocr
// #include "locr.h"
import "C"

var out *C.char
status := C.locr_image_to_text((*C.uint8_t)(&data[0]), C.size_t(len(data)), &out)
defer C.locr_free_text(out)
```

### C++

```cpp
#include "locr.h"
char *text = nullptr;
if (locr_image_to_text(bytes.data(), bytes.size(), &text) == LOCR_OK) {
    std::string result(text);
    locr_free_text(text);
}
```

## Build

```bash
cargo build -p locr-ffi --release
# -> target/release/locr.dll | liblocr.so | liblocr.dylib (+ static .a/.lib)
```

Prebuilt binaries for Linux/macOS/Windows (x64 + ARM64) ship on every release
and as CI artifacts.

## ABI stability

This surface is frozen under semver. New functions may be added; existing
signatures never change without a major version bump.
