/* Rank 0 opens the corpus read-only, packs one frame as RCSO, and
 * MPI_Bcast the blob. Other ranks never open LMDB.
 *
 *   mpicc -DREADCON_HAVE_MPI -Iinclude mpi_bcast_frame.c -lreadcon_db
 *
 * Without MPI the same pack/unpack still works on one process.
 */
#include "readcon-db.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

#ifdef READCON_HAVE_MPI
#include <mpi.h>
#endif

int main(int argc, char **argv) {
    int rank = 0;
#ifdef READCON_HAVE_MPI
    MPI_Init(&argc, &argv);
    MPI_Comm_rank(MPI_COMM_WORLD, &rank);
#endif
    if (argc < 2) {
        if (rank == 0)
            fprintf(stderr, "usage: %s <corpus_dir> [traj] [frame]\n", argv[0]);
#ifdef READCON_HAVE_MPI
        MPI_Finalize();
#endif
        return 1;
    }
    const char *dir = argv[1];
    uint64_t traj = argc > 2 ? (uint64_t)strtoull(argv[2], NULL, 10) : 1;
    uint32_t frame = argc > 3 ? (uint32_t)strtoul(argv[3], NULL, 10) : 0;

    uint8_t *buf = NULL;
    int nbytes = 0;
    if (rank == 0) {
        size_t id = 0;
        if (rkrdb_open_readonly(dir, &id) != RKRDB_OK) {
            fprintf(stderr, "open_readonly failed\n");
#ifdef READCON_HAVE_MPI
            MPI_Abort(MPI_COMM_WORLD, 2);
#endif
            return 2;
        }
        buf = (uint8_t *)malloc(1 << 20);
        nbytes = rkrdb_pack_frame(id, traj, frame, buf, 1 << 20);
        rkrdb_close(id);
        if (nbytes < 0) {
            fprintf(stderr, "pack_frame failed\n");
            free(buf);
#ifdef READCON_HAVE_MPI
            MPI_Abort(MPI_COMM_WORLD, 3);
#endif
            return 3;
        }
    }
#ifdef READCON_HAVE_MPI
    MPI_Bcast(&nbytes, 1, MPI_INT, 0, MPI_COMM_WORLD);
    if (rank != 0)
        buf = (uint8_t *)malloc((size_t)nbytes);
    MPI_Bcast(buf, nbytes, MPI_BYTE, 0, MPI_COMM_WORLD);
#endif
    uint32_t natoms = 0;
    double xyz[3 * 4096];
    if (rkrdb_unpack_positions(buf, (size_t)nbytes, xyz, 4096, &natoms) != RKRDB_OK) {
        fprintf(stderr, "rank %d unpack failed\n", rank);
        free(buf);
#ifdef READCON_HAVE_MPI
        MPI_Finalize();
#endif
        return 4;
    }
    if (rank == 0)
        printf("bcast %d bytes, natoms=%u xyz0=(%.4f,%.4f,%.4f)\n", nbytes, natoms,
               xyz[0], xyz[1], xyz[2]);
    free(buf);
#ifdef READCON_HAVE_MPI
    MPI_Finalize();
#endif
    return 0;
}
