#ifndef RUN_ID_H
#define RUN_ID_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

uint32_t murmur3_32(const char *key, uint32_t seed);
unsigned long long make_run_id(void);

#ifdef __cplusplus
}
#endif

#endif
