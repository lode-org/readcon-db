/* Rank 0 of the *caller* communicator opens the corpus read-only, packs
 * one frame as RCSO, and broadcasts the blob on that communicator. Other
 * ranks never open LMDB.
 *
 * Hosts that already own MPI (LAMMPS pair/fix, mpi4py, a plugin) pass the
 * communicator they already use — lmp->world, a sub-comm, a Dup/Split —
 * and must not let this file call MPI_Init. This standalone main Dups the
 * process-wide world handle only because it *is* the host; the helper
 * never names that handle.
 *
 *   mpicc -DREADCON_HAVE_MPI -Iinclude examples/mpi_bcast_frame.c -lreadcon_db
 *
 * Without MPI the same pack/unpack still works on one process.
 */
#include "readcon-db.h"
#ifdef READCON_HAVE_MPI
#include "readcon-db-mpi.h"
#endif
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

int main(int argc, char **argv) {
    int rank = 0;
#ifdef READCON_HAVE_MPI
    int already = 0;
    int we_inited = 0;
    MPI_Comm comm = MPI_COMM_NULL;
    MPI_Initialized(&already);
    if (!already) {
        MPI_Init(&argc, &argv);
        we_inited = 1;
    }
    /* Host-owned comm. Standalone: Dup the process-wide handle so the
     * helper sees a caller comm, not that handle. LAMMPS passes lmp->world
     * here instead and never reaches this Dup. */
    if (MPI_Comm_dup(MPI_COMM_WORLD, &comm) != MPI_SUCCESS) {
        fprintf(stderr, "MPI_Comm_dup failed\n");
        if (we_inited)
            MPI_Finalize();
        return 1;
    }
    MPI_Comm_rank(comm, &rank);
#endif
    if (argc < 2) {
        if (rank == 0)
            fprintf(stderr, "usage: %s <corpus_dir> [traj] [frame]\n", argv[0]);
#ifdef READCON_HAVE_MPI
        MPI_Comm_free(&comm);
        if (we_inited)
            MPI_Finalize();
#endif
        return 1;
    }
    const char *dir = argv[1];
    uint64_t traj = argc > 2 ? (uint64_t)strtoull(argv[2], NULL, 10) : 1;
    uint32_t frame = argc > 3 ? (uint32_t)strtoul(argv[3], NULL, 10) : 0;

    uint8_t *buf = NULL;
    int nbytes = 0;
#ifdef READCON_HAVE_MPI
    int st = rkrdb_bcast_packed_frame(comm, 0, dir, traj, frame, &buf, &nbytes);
    if (st != RKRDB_OK) {
        if (rank == 0)
            fprintf(stderr, "bcast_packed_frame failed (%d)\n", st);
        free(buf);
        MPI_Comm_free(&comm);
        if (we_inited)
            MPI_Finalize();
        return 2;
    }
    {
        uint8_t *batch = NULL;
        int bn = 0;
        uint64_t tids[1] = {traj};
        uint32_t fids[1] = {frame};
        int st2 = rkrdb_bcast_packed_frames(comm, 0, dir, tids, fids, 1, &batch, &bn);
        if (st2 != RKRDB_OK) {
            if (rank == 0)
                fprintf(stderr, "bcast_packed_frames failed (%d)\n", st2);
            free(buf);
            free(batch);
            MPI_Comm_free(&comm);
            if (we_inited)
                MPI_Finalize();
            return 2;
        }
        {
            uint32_t nfr = 0, na2 = 0;
            double xyz2[3 * 4096];
            if (rkrdb_unpack_batch_nframes(batch, (size_t)bn, &nfr) != RKRDB_OK ||
                rkrdb_unpack_batch_item(batch, (size_t)bn, 0, xyz2, 4096, &na2) !=
                    RKRDB_OK) {
                if (rank == 0)
                    fprintf(stderr, "unpack_batch failed\n");
                free(buf);
                free(batch);
                MPI_Comm_free(&comm);
                if (we_inited)
                    MPI_Finalize();
                return 2;
            }
        }
        free(batch);
    }
#else
    size_t id = 0;
    if (rkrdb_open_readonly(dir, &id) != RKRDB_OK) {
        fprintf(stderr, "open_readonly failed\n");
        return 2;
    }
    buf = (uint8_t *)malloc(1 << 20);
    nbytes = rkrdb_pack_frame(id, traj, frame, buf, 1 << 20);
    rkrdb_close(id);
    if (nbytes < 0) {
        fprintf(stderr, "pack_frame failed\n");
        free(buf);
        return 3;
    }
#endif
    uint32_t natoms = 0;
    double xyz[3 * 4096];
    if (rkrdb_unpack_positions(buf, (size_t)nbytes, xyz, 4096, &natoms) != RKRDB_OK) {
        fprintf(stderr, "rank %d unpack failed\n", rank);
        free(buf);
#ifdef READCON_HAVE_MPI
        MPI_Comm_free(&comm);
        if (we_inited)
            MPI_Finalize();
#endif
        return 4;
    }
    if (rank == 0)
        printf("bcast %d bytes, natoms=%u xyz0=(%.4f,%.4f,%.4f)\n", nbytes, natoms,
               xyz[0], xyz[1], xyz[2]);
    free(buf);
#ifdef READCON_HAVE_MPI
    MPI_Comm_free(&comm);
    if (we_inited)
        MPI_Finalize();
#endif
    return 0;
}
