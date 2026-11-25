#define _POSIX_C_SOURCE 200809L

#include "readcon-db.h"

#include <dirent.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

/* More digits than the shortest binary64 spelling: a parse and rewrite
 * changes this text, so only a hash of the source span matches the store. */
static const char HIGHPREC_CON[] =
    "Random Number Seed\n"
    "{\"con_spec_version\":2}\n"
    "15.345600123456789 21.702000987654321 100.000000000123456\n"
    "90.0 90.0 90.0\n"
    "0 0\n"
    "218 0 1\n"
    "2\n"
    "1 1\n"
    "1000000.123456789 0.00000000123456789\n"
    "Cu\n"
    "Coordinates of Component 1\n"
    "1.23456789012345 0.90450000000000019 6.97529999999999539 1 0\n"
    "H\n"
    "Coordinates of Component 2\n"
    "0.00000000123456789 9.94699999999999740 11.73299999999999343 0 1\n";

static void remove_tree(const char *dir) {
    DIR *d = opendir(dir);
    if (d) {
        struct dirent *e;
        char path[4096];
        while ((e = readdir(d)) != NULL) {
            if (strcmp(e->d_name, ".") == 0 || strcmp(e->d_name, "..") == 0)
                continue;
            snprintf(path, sizeof(path), "%s/%s", dir, e->d_name);
            unlink(path);
        }
        closedir(d);
    }
    rmdir(dir);
}

static int check_hash_con_text(void) {
    const char *tmp = getenv("TMPDIR");
    char dir[4096];
    snprintf(dir, sizeof(dir), "%s/rkrdb-hash-XXXXXX", (tmp && *tmp) ? tmp : "/tmp");
    if (!mkdtemp(dir)) {
        perror("mkdtemp");
        return 1;
    }
    size_t id = 0;
    uint32_t n = 0;
    uint8_t stored[16];
    uint8_t text_hash[16];
    int rc = 1;
    if (rkrdb_open(dir, &id) != RKRDB_OK) {
        fprintf(stderr, "rkrdb_open failed\n");
        remove_tree(dir);
        return 1;
    }
    if (rkrdb_append_trajectory_str(id, 1, HIGHPREC_CON, NULL, &n) != RKRDB_OK || n != 1) {
        fprintf(stderr, "rkrdb_append_trajectory_str failed\n");
    } else if (rkrdb_frame_hash(id, 1, 0, stored) != RKRDB_OK) {
        fprintf(stderr, "rkrdb_frame_hash failed\n");
    } else if (rkrdb_hash_con_text(HIGHPREC_CON, strlen(HIGHPREC_CON), text_hash) != RKRDB_OK) {
        fprintf(stderr, "rkrdb_hash_con_text failed\n");
    } else if (memcmp(stored, text_hash, sizeof(stored)) != 0) {
        fprintf(stderr, "rkrdb_hash_con_text differs from the stored hash\n");
    } else {
        rc = 0;
    }
    rkrdb_close(id);
    remove_tree(dir);
    if (rc == 0)
        printf("rkrdb_hash_con_text ok\n");
    return rc;
}

int main(void) {
    const uint8_t data[] = "readcon-db";
    uint8_t hash[16];
    memset(hash, 0, sizeof(hash));
    if (rkrdb_xxh3_128(data, sizeof(data) - 1, hash) != RKRDB_OK) {
        fprintf(stderr, "rkrdb_xxh3_128 failed\n");
        return 1;
    }
    int nonzero = 0;
    for (int i = 0; i < 16; i++) {
        if (hash[i] != 0) {
            nonzero = 1;
            break;
        }
    }
    if (!nonzero) {
        fprintf(stderr, "rkrdb_xxh3_128 returned an all-zero digest\n");
        return 1;
    }
    printf("rkrdb_xxh3_128 ok\n");
    return check_hash_con_text();
}
