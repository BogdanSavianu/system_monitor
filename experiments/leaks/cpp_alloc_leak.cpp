#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <ctime>
#include <string>
#include <vector>

#include <unistd.h>

#include "run_id.h"

#define NEW_ARRAY_KB 32
#define NEW_OBJECT_KB 128
#define VECTOR_KB 96
#define STRING_KB 48
#define STEP_LEAK_KB (NEW_ARRAY_KB + NEW_OBJECT_KB + VECTOR_KB + STRING_KB)

// just so the allocations do not get optimized away
static volatile void *sink;

static void touch(void *p, size_t bytes) {
    char *b = (char *)(p);
    for (size_t off = 0; off < bytes; off += 4096) {
        b[off] = (char)(off & 0xFF);
    }
}

struct Block {
    char data[NEW_OBJECT_KB * 1024];
};

static void *leak_new_array(void) {
    char *p = new char[NEW_ARRAY_KB * 1024];  // operator new[]
    touch(p, NEW_ARRAY_KB * 1024);
    return p;
}

static void *leak_new_object(void) {
    Block *b = new Block();  // operator new
    touch(b->data, sizeof(b->data));
    return b;
}

static void *leak_vector(void) {
    auto *v = new std::vector<char>(VECTOR_KB * 1024, 7);  // new -> malloc
    touch(v->data(), v->size());
    return v;
}

static void *leak_string(void) {
    auto *s = new std::string(STRING_KB * 1024, 'x');  // string buffer on the heap
    touch(&(*s)[0], s->size());
    return s;
}

static uint64_t parse_u64(const char *s, const char *name) {
    unsigned long long v = 0;
    char tail = '\0';
    if (sscanf(s, "%llu%c", &v, &tail) != 1) {
        fprintf(stderr, "invalid %s: %s\n", name, s);
        exit(1);
    }
    return (uint64_t)(v);
}

int main(int argc, char **argv) {
    if (argc != 3 && argc != 4) {
        fprintf(stderr, "usage: %s <interval_s> <steps_0_forever> [output_csv]\n", argv[0]);
        return 1;
    }

    uint64_t interval_s = parse_u64(argv[1], "interval_s");
    uint64_t steps = parse_u64(argv[2], "steps");

    const char *scenario = "cpp_alloc";
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
        sink = leak_new_array();
        sink = leak_new_object();
        sink = leak_vector();
        sink = leak_string();

        total_kb += STEP_LEAK_KB;
        if ((i % 10) == 0) {
            printf("step=%llu leaked_kb=%llu (one block per C++ site)\n",
                   (unsigned long long)(i),
                   (unsigned long long)(total_kb));
            fflush(stdout);
        }

        if (csv) {
            fprintf(csv, "%s,%d,%llu,%llu,%llu,%llu,%llu,%llu\n",
                scenario,
                label,
                run_id,
                (unsigned long long)(i),
                (unsigned long long)(i * interval_s),
                (unsigned long long)(STEP_LEAK_KB),
                (unsigned long long)(total_kb),
                (unsigned long long)(STEP_LEAK_KB));
            fflush(csv);
        }

        i++;
        sleep((unsigned int)(interval_s));
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
                fprintf(meta, "start_epoch_s=%lld\n", (long long)(start_epoch_s));
                fprintf(meta, "end_epoch_s=%lld\n", (long long)(end_epoch_s));
                fprintf(meta, "interval_s=%llu\n", (unsigned long long)(interval_s));
                fprintf(meta, "steps=%llu\n", (unsigned long long)(steps));
                fclose(meta);
            }
        }
    }

    return 0;
}
