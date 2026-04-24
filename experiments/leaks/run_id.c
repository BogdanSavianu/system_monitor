#include "run_id.h"

#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <unistd.h>

static uint32_t rotl32(uint32_t x, uint8_t r) {
    return (x << r) | (x >> (32 - r));
}

uint32_t murmur3_32(const char *key, uint32_t seed) {
    const uint8_t *data = (const uint8_t *)key;
    size_t len = strlen(key);
    uint32_t h = seed;
    const uint32_t c1 = 0xcc9e2d51U;
    const uint32_t c2 = 0x1b873593U;

    size_t blocks = len / 4;
    for (size_t i = 0; i < blocks; i++) {
        uint32_t k = (uint32_t)data[i * 4] |
                     ((uint32_t)data[i * 4 + 1] << 8) |
                     ((uint32_t)data[i * 4 + 2] << 16) |
                     ((uint32_t)data[i * 4 + 3] << 24);
        k *= c1;
        k = rotl32(k, 15);
        k *= c2;

        h ^= k;
        h = rotl32(h, 13);
        h = h * 5U + 0xe6546b64U;
    }

    const uint8_t *tail = data + blocks * 4;
    size_t tail_len = len & 3U;
    if (tail_len != 0U) {
        uint32_t k1 = 0;
        if (tail_len > 2U) {
            k1 ^= (uint32_t)tail[2] << 16;
        }
        if (tail_len > 1U) {
            k1 ^= (uint32_t)tail[1] << 8;
        }
        k1 ^= (uint32_t)tail[0];
        k1 *= c1;
        k1 = rotl32(k1, 15);
        k1 *= c2;
        h ^= k1;
    }

    h ^= (uint32_t)len;
    h ^= h >> 16;
    h *= 0x85ebca6bU;
    h ^= h >> 13;
    h *= 0xc2b2ae35U;
    h ^= h >> 16;
    return h;
}

unsigned long long make_run_id(void) {
    static uint64_t pid_mix = 0;
    static uint64_t env_mix = 0;
    static int initialized = 0;

    if (!initialized) {
        pid_mix = (uint64_t)getpid() << 32;

        const char *ext_seed = getenv("LEAK_RUN_SEED");
        if (ext_seed != NULL && *ext_seed != '\0') {
            env_mix = (uint64_t)murmur3_32(ext_seed, 0x9747b28cU);
        }
        initialized = 1;
    }

    struct timeval tv;
    gettimeofday(&tv, NULL);

    //convert to microseconds
    uint64_t time_mix = (uint64_t)tv.tv_sec * 1000000ULL + (uint64_t)tv.tv_usec;
    return (unsigned long long)(time_mix ^ pid_mix ^ env_mix);
}
