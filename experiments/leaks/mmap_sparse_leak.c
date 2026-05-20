#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

#include "run_id.h"

static uint64_t parse_u64(const char *s, const char *name) {
    unsigned long long v = 0;
    char tail = '\0';
    if (sscanf(s, "%llu%c", &v, &tail) != 1) {
        fprintf(stderr, "invalid %s: %s\n", name, s);
        exit(1);
    }
    return (uint64_t)v;
}

int main(int argc, char **argv) {
    if (argc != 6 && argc != 7) {
        fprintf(stderr, "usage: %s <regions_per_step> <region_kb> <release_every_steps> <interval_s> <steps_0_forever> [output_csv]\n", argv[0]);
        return 1;
    }

    uint64_t regions_per_step = parse_u64(argv[1], "regions_per_step");
    uint64_t region_kb = parse_u64(argv[2], "region_kb");
    uint64_t release_every = parse_u64(argv[3], "release_every_steps");
    uint64_t interval_s = parse_u64(argv[4], "interval_s");
    uint64_t steps = parse_u64(argv[5], "steps_0_forever");

    if (regions_per_step == 0 || region_kb == 0 || release_every == 0) {
        fprintf(stderr, "regions_per_step, region_kb and release_every_steps must be > 0\n");
        return 1;
    }

    const char *scenario = "mmap_sparse_leak";
    const int label = 1;
    time_t start_epoch_s = time(NULL);
    unsigned long long run_id = make_run_id();

    FILE *csv = NULL;
    if (argc == 7) {
        csv = fopen(argv[6], "w");
        if (!csv) {
            fprintf(stderr, "failed to open csv file: %s\n", argv[6]);
            return 1;
        }
        fprintf(csv, "scenario,label,run_id,step,elapsed_s,leaked_kb_step,leaked_kb_total,workload_kb_this_step\n");
    }

    void **regions = NULL;
    size_t regions_count = 0;
    size_t regions_capacity = 0;
    size_t bytes_per_region = (size_t)(region_kb * 1024ULL);

    uint64_t step = 0;
    uint64_t leaked_total_kb = 0;

    while (steps == 0 || step < steps) {
        for (uint64_t i = 0; i < regions_per_step; i++) {
            void *p = mmap(NULL, bytes_per_region, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
            if (p == MAP_FAILED) {
                fprintf(stderr, "mmap failed at step=%llu\n", (unsigned long long)step);
                return 2;
            }

            /* Touch one byte per page so pages become resident. */
            size_t page = 4096;
            unsigned char *b = (unsigned char *)p;
            for (size_t off = 0; off < bytes_per_region; off += page) {
                b[off] = (unsigned char)(off & 0xFF);
            }

            if (regions_count == regions_capacity) {
                size_t next_capacity = regions_capacity == 0 ? 1024 : regions_capacity * 2;
                void **next = realloc(regions, next_capacity * sizeof(void *));
                if (!next) {
                    fprintf(stderr, "realloc failed while tracking mmap regions\n");
                    return 2;
                }
                regions = next;
                regions_capacity = next_capacity;
            }
            regions[regions_count++] = p;
        }

        uint64_t leaked_this_step_kb = regions_per_step * region_kb;
        leaked_total_kb += leaked_this_step_kb;

        /* Occasionally unmap a portion to add non-monotonic behavior. */
        if ((step % release_every) == 0 && regions_count > regions_per_step * 4) {
            size_t to_release = regions_count / 6;
            for (size_t i = 0; i < to_release; i++) {
                munmap(regions[i], bytes_per_region);
            }
            memmove(regions, regions + to_release, (regions_count - to_release) * sizeof(void *));
            regions_count -= to_release;
            uint64_t released_kb = (uint64_t)to_release * region_kb;
            if (released_kb <= leaked_total_kb) {
                leaked_total_kb -= released_kb;
            } else {
                leaked_total_kb = 0;
            }
        }

        if ((step % 5) == 0) {
            printf("step=%llu leaked_kb_step=%llu leaked_kb_total=%llu workload_kb_this_step=%llu\n",
                   (unsigned long long)step,
                   (unsigned long long)leaked_this_step_kb,
                   (unsigned long long)leaked_total_kb,
                   (unsigned long long)(regions_per_step * region_kb));
            fflush(stdout);
        }

        if (csv) {
            fprintf(csv, "%s,%d,%llu,%llu,%llu,%llu,%llu,%llu\n",
                    scenario,
                    label,
                    run_id,
                    (unsigned long long)step,
                    (unsigned long long)(step * interval_s),
                    (unsigned long long)leaked_this_step_kb,
                    (unsigned long long)leaked_total_kb,
                    (unsigned long long)(regions_per_step * region_kb));
            fflush(csv);
        }

        step++;
        sleep((unsigned int)interval_s);
    }

    if (csv) {
        fclose(csv);

        char meta_path[1024];
        if (snprintf(meta_path, sizeof(meta_path), "%s.meta", argv[6]) > 0) {
            FILE *meta = fopen(meta_path, "w");
            if (meta) {
                time_t end_epoch_s = time(NULL);
                fprintf(meta, "scenario=%s\n", scenario);
                fprintf(meta, "label=%d\n", label);
                fprintf(meta, "run_id=%llu\n", run_id);
                fprintf(meta, "start_epoch_s=%lld\n", (long long)start_epoch_s);
                fprintf(meta, "end_epoch_s=%lld\n", (long long)end_epoch_s);
                fprintf(meta, "regions_per_step=%llu\n", (unsigned long long)regions_per_step);
                fprintf(meta, "region_kb=%llu\n", (unsigned long long)region_kb);
                fprintf(meta, "release_every_steps=%llu\n", (unsigned long long)release_every);
                fprintf(meta, "interval_s=%llu\n", (unsigned long long)interval_s);
                fprintf(meta, "steps=%llu\n", (unsigned long long)steps);
                fclose(meta);
            }
        }
    }

    return 0;
}
