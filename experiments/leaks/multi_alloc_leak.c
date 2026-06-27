#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

#include "run_id.h"

#define SMALL_MALLOC_KB 16
#define LARGE_MALLOC_KB 128
#define CALLOC_KB 64
#define REALLOC_KB 96
#define STEP_LEAK_KB (SMALL_MALLOC_KB + LARGE_MALLOC_KB + CALLOC_KB + REALLOC_KB)

// just so the allocations do not get optimized
static volatile void *sink;

static void touch(void *p, size_t bytes) {
    char *b = (char *)p;
    for (size_t off = 0; off < bytes; off += 4096) {
        b[off] = (char)(off & 0xFF);
    }
}

static void *leak_small_malloc(void) {
    void *p = malloc(SMALL_MALLOC_KB * 1024);
    if (p) touch(p, SMALL_MALLOC_KB * 1024);
    return p;
}

static void *leak_large_malloc(void) {
    void *p = malloc(LARGE_MALLOC_KB * 1024);
    if (p) touch(p, LARGE_MALLOC_KB * 1024);
    return p;
}

static void *leak_calloc(void) {
    void *p = calloc(CALLOC_KB, 1024);
    if (p) touch(p, CALLOC_KB * 1024);
    return p;
}

static void *leak_realloc(void) {
    void *p = malloc((REALLOC_KB / 2) * 1024);
    p = realloc(p, REALLOC_KB * 1024);
    if (p) touch(p, REALLOC_KB * 1024);
    return p;
}

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
    if (argc != 3 && argc != 4) {
        fprintf(stderr, "usage: %s <interval_s> <steps_0_forever> [output_csv]\n", argv[0]);
        return 1;
    }

    uint64_t interval_s = parse_u64(argv[1], "interval_s");
    uint64_t steps = parse_u64(argv[2], "steps");

    const char *scenario = "multi_alloc_leak";
    const int label = 1;
    time_t start_epoch_s = time(NULL);
    unsigned long long run_id = make_run_id();

    FILE *csv = NULL;
    if (argc == 4) {
        csv = fopen(argv[3], "w");
        if (!csv) {
            fprintf(stderr, "failed to open csv file: %s\n", argv[3]);
            return 1;
        }
        fprintf(csv, "scenario,label,run_id,step,elapsed_s,leaked_kb_step,leaked_kb_total,workload_kb_this_step\n");
    }

    uint64_t i = 0;
    uint64_t total_kb = 0;
    while (steps == 0 || i < steps) {
        sink = leak_small_malloc();
        sink = leak_large_malloc();
        sink = leak_calloc();
        sink = leak_realloc();

        total_kb += STEP_LEAK_KB;
        if ((i % 10) == 0) {
            printf("step=%llu leaked_kb=%llu (one block per site A-E)\n",
                   (unsigned long long)i,
                   (unsigned long long)total_kb);
            fflush(stdout);
        }

        if (csv) {
            fprintf(csv, "%s,%d,%llu,%llu,%llu,%llu,%llu,%llu\n",
                scenario,
                label,
                run_id,
                (unsigned long long)i,
                (unsigned long long)(i * interval_s),
                (unsigned long long)STEP_LEAK_KB,
                (unsigned long long)total_kb,
                (unsigned long long)STEP_LEAK_KB);
            fflush(csv);
        }

        i++;
        sleep((unsigned int)interval_s);
    }

    if (csv) {
        fclose(csv);

        char meta_path[1024];
        if (snprintf(meta_path, sizeof(meta_path), "%s.meta", argv[3]) > 0) {
            FILE *meta = fopen(meta_path, "w");
            if (meta) {
                time_t end_epoch_s = time(NULL);
                fprintf(meta, "scenario=%s\n", scenario);
                fprintf(meta, "label=%d\n", label);
                fprintf(meta, "run_id=%llu\n", run_id);
                fprintf(meta, "start_epoch_s=%lld\n", (long long)start_epoch_s);
                fprintf(meta, "end_epoch_s=%lld\n", (long long)end_epoch_s);
                fprintf(meta, "interval_s=%llu\n", (unsigned long long)interval_s);
                fprintf(meta, "steps=%llu\n", (unsigned long long)steps);
                fclose(meta);
            }
        }
    }

    return 0;
}
