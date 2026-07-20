#include <dlfcn.h>
#include <stdint.h>
#include <stdio.h>

typedef int32_t (*panic_boundary_fn)(void);

int main(int argc, char **argv) {
    if (argc != 2) {
        return 2;
    }
    void *library = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (library == NULL) {
        fprintf(stderr, "%s\n", dlerror());
        return 3;
    }
    panic_boundary_fn boundary = (panic_boundary_fn)dlsym(library, "ad_test_panic_boundary");
    if (boundary == NULL) {
        fprintf(stderr, "%s\n", dlerror());
        dlclose(library);
        return 4;
    }
    int32_t result = boundary();
    dlclose(library);
    if (result != -12) {
        fprintf(stderr, "unexpected result: %d\n", result);
        return 5;
    }
    return 0;
}
