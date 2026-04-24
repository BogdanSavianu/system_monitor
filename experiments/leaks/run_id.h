#ifndef RUN_ID_H
#define RUN_ID_H

#include <stdint.h>

uint32_t murmur3_32(const char *key, uint32_t seed);
unsigned long long make_run_id(void);

#endif
