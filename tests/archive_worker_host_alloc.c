/* Round-trip one archive row through the static archives.
 * Linking libreadcon_db.a beside libreadcon_core.a must free with one
 * allocator. Two copies of std, one of them mimalloc, abort here in
 * free() or mi_free on the archive worker. */
#include "readcon-db.h"

#include <stdio.h>
#include <stdlib.h>

int main(void)
{
    const char *dir = getenv("ARCHIVE_DIR");
    const uint32_t z[2] = {1, 1};
    const double cell[3] = {25.0, 25.0, 25.0};
    const double positions[6] = {0.0, 0.0, 0.0, 1.0, 0.0, 0.0};
    const double forces[6] = {-0.1, -0.1, -0.1, -0.1, -0.1, -0.1};
    double out_positions[6];
    double out_forces[6];
    double energy = 0.0;
    size_t id = 0;

    if (dir == NULL) {
        fprintf(stderr, "ARCHIVE_DIR is unset\n");
        return 2;
    }
    if (rkrdb_archive_open(dir, z, 2, cell, &id) != RKRDB_OK) {
        fprintf(stderr, "rkrdb_archive_open failed\n");
        return 3;
    }
    if (rkrdb_archive_append(id, positions, forces, -1.5) != RKRDB_OK) {
        fprintf(stderr, "rkrdb_archive_append failed\n");
        return 4;
    }
    if (rkrdb_archive_flush(id) != RKRDB_OK) {
        fprintf(stderr, "rkrdb_archive_flush failed\n");
        return 5;
    }
    if (rkrdb_archive_fetch(id, 0, out_positions, out_forces, 2, &energy) !=
        RKRDB_OK) {
        fprintf(stderr, "rkrdb_archive_fetch failed\n");
        return 6;
    }
    if (rkrdb_archive_close(id) != RKRDB_OK) {
        fprintf(stderr, "rkrdb_archive_close failed\n");
        return 7;
    }
    if (energy < -1.5000001 || energy > -1.4999999) {
        fprintf(stderr, "energy %g\n", energy);
        return 8;
    }
    return 0;
}
