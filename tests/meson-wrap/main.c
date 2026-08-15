#include "readcon-db.h"

#include <stdint.h>
#include <stdio.h>
#include <string.h>

int main(void) {
    const uint8_t data[] = "readcon-db";
    uint8_t hash[16];
    memset(hash, 0, sizeof(hash));
    if (rkrdb_xxh3_128(data, sizeof(data) - 1, hash) != RKRDB_OK) {
        fprintf(stderr, "rkrdb_xxh3_128 failed\n");
        return 1;
    }
    printf("rkrdb_xxh3_128 ok\n");
    return 0;
}
