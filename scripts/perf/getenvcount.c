// LD_PRELOAD shim: counts getenv/secure_getenv calls per name, dumps "count name" lines to $GETENV_COUNT_OUT at exit.
// gcc -O2 -shared -fPIC -o getenvcount.so getenvcount.c -ldl; GETENV_COUNT_OUT=out.txt LD_PRELOAD=./getenvcount.so game --bench racing 600
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <pthread.h>
#define N 4096
static char *names[N]; static unsigned long counts[N]; static int used;
static pthread_mutex_t mu = PTHREAD_MUTEX_INITIALIZER;
static char *(*real_getenv)(const char *);
static char *(*real_sgetenv)(const char *);
static void count(const char *name) {
  pthread_mutex_lock(&mu);
  for (int i = 0; i < used; i++) if (strcmp(names[i], name) == 0) { counts[i]++; pthread_mutex_unlock(&mu); return; }
  if (used < N) { names[used] = strdup(name); counts[used++] = 1; }
  pthread_mutex_unlock(&mu);
}
char *getenv(const char *name) { if (!real_getenv) real_getenv = dlsym(RTLD_NEXT, "getenv"); count(name); return real_getenv(name); }
char *secure_getenv(const char *name) { if (!real_sgetenv) real_sgetenv = dlsym(RTLD_NEXT, "secure_getenv"); count(name); return real_sgetenv(name); }
__attribute__((destructor)) static void dump(void) {
  const char *out = real_getenv ? real_getenv("GETENV_COUNT_OUT") : NULL;
  FILE *f = out ? fopen(out, "w") : stderr; if (!f) return;
  for (int i = 0; i < used; i++) fprintf(f, "%lu %s\n", counts[i], names[i]);
  if (f != stderr) fclose(f);
}
