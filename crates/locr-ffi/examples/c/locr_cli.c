/*
 * Minimal C consumer of locr.h: prints the text, then the JSON result.
 *
 *   cargo build -p locr-ffi --release
 *   cc -I crates/locr-ffi/include crates/locr-ffi/examples/c/locr_cli.c \
 *      -L target/release -llocr -o locr_cli
 *   LD_LIBRARY_PATH=target/release ./locr_cli image.png
 */
#include <stdio.h>
#include <stdlib.h>

#include "locr.h"

static unsigned char *read_file(const char *path, size_t *len) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    unsigned char *buf = size > 0 ? malloc((size_t)size) : NULL;
    if (buf && fread(buf, 1, (size_t)size, f) != (size_t)size) {
        free(buf);
        buf = NULL;
    }
    fclose(f);
    *len = buf ? (size_t)size : 0;
    return buf;
}

int main(int argc, char **argv) {
    if (argc != 2) {
        fprintf(stderr, "usage: %s IMAGE\n", argv[0]);
        return 2;
    }
    size_t len = 0;
    unsigned char *bytes = read_file(argv[1], &len);
    if (!bytes) {
        fprintf(stderr, "cannot read %s\n", argv[1]);
        return 1;
    }

    printf("locr %s\n", locr_version());

    char *text = NULL;
    LocrStatus status = locr_image_to_text(bytes, len, &text);
    if (status != LOCR_OK) {
        fprintf(stderr, "locr_image_to_text: %d %s\n", status, locr_last_error());
        free(bytes);
        return 1;
    }
    printf("text: %s\n", text);
    locr_free_text(text);

    char *json = NULL;
    status = locr_image_to_json(bytes, len, "{\"beam_width\": 3}", &json);
    if (status != LOCR_OK) {
        fprintf(stderr, "locr_image_to_json: %d %s\n", status, locr_last_error());
        free(bytes);
        return 1;
    }
    printf("json: %s\n", json);
    locr_free_text(json);

    /* Errors carry a message. */
    status = locr_image_to_json(bytes, len, "{\"typo\": 1}", &json);
    printf("bad options -> %d (%s)\n", status, locr_last_error());

    free(bytes);
    return status == LOCR_INVALID_INPUT && json == NULL ? 0 : 1;
}
