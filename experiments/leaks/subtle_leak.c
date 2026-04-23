#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

uint64_t parse_u64(const char *s, const char *name) {
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
        fprintf(stderr, "usage: %s <allocs_per_step> <kb_per_alloc> <leak_percent_0_100> <interval_s> <steps_0_forever> [output_csv]\n", argv[0]);
        return 1;
    }

    uint64_t allocs = parse_u64(argv[1], "allocs_per_step");
    uint64_t kb_per_alloc = parse_u64(argv[2], "kb_per_alloc");
    uint64_t leak_percent = parse_u64(argv[3], "leak_percent");
    uint64_t interval_s = parse_u64(argv[4], "interval_s");
    uint64_t steps = parse_u64(argv[5], "steps");
    const char *scenario = "subtle_leak";
    const int label = 1;
    time_t start_epoch_s = time(NULL);
    unsigned long long run_id =
        (unsigned long long)start_epoch_s ^ (unsigned long long)getpid();

    FILE *csv = NULL;
    if (argc == 7) {
        csv = fopen(argv[6], "w");
        if (!csv) {
            fprintf(stderr, "failed to open csv file: %s\n", argv[6]);
            return 1;
        }
        fprintf(csv, "scenario,label,run_id,step,elapsed_s,leaked_kb_step,leaked_kb_total,workload_kb_this_step\n");
    }

    if (leak_percent > 100) {
        fprintf(stderr, "leak_percent must be <= 100\n");
        return 1;
    }

    size_t bytes = (size_t)(kb_per_alloc * 1024ULL);
    uint64_t i = 0;
    uint64_t total_leaked_kb = 0;

    while (steps == 0 || i < steps) {
        uint64_t leaked_this_step_kb = 0;
        for (uint64_t j = 0; j < allocs; j++) {
            void *p = malloc(bytes);
            if (!p) {
                fprintf(stderr, "malloc failed at step=%llu alloc=%llu\n",
                        (unsigned long long)i,
                        (unsigned long long)j);
                return 2;
            }
            memset(p, 0xC3, bytes);

            uint64_t threshold = (j % 100);
            if (threshold < leak_percent) {
                total_leaked_kb += kb_per_alloc;
                leaked_this_step_kb += kb_per_alloc;
            } else {
                free(p);
            }
        }

        if ((i % 5) == 0) {
            printf("step=%llu leaked_kb_total=%llu\n",
                   (unsigned long long)i,
                   (unsigned long long)total_leaked_kb);
            fflush(stdout);
        }

        if (csv) {
            fprintf(csv, "%s,%d,%llu,%llu,%llu,%llu,%llu,%llu\n",
                scenario,
                label,
                run_id,
                    (unsigned long long)i,
                    (unsigned long long)(i * interval_s),
                    (unsigned long long)leaked_this_step_kb,
                (unsigned long long)total_leaked_kb,
                (unsigned long long)(allocs * kb_per_alloc));
            fflush(csv);
        }

        i++;
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
                fprintf(meta, "allocs_per_step=%llu\n", (unsigned long long)allocs);
                fprintf(meta, "kb_per_alloc=%llu\n", (unsigned long long)kb_per_alloc);
                fprintf(meta, "leak_percent=%llu\n", (unsigned long long)leak_percent);
                fprintf(meta, "interval_s=%llu\n", (unsigned long long)interval_s);
                fprintf(meta, "steps=%llu\n", (unsigned long long)steps);
                fclose(meta);
            }
        }
    }

    return 0;
}
