#include <stdio.h>
#include <stdint.h>
#define BE_LOG_MSG_ERROR 0
#define BE_LOG_MSG_NORMAL 1
#define BE_Cross_LogMessage(l,...) fprintf(stderr,__VA_ARGS__)
static inline int32_t BE_Cross_FileLengthFromHandle(FILE*f){long p=ftell(f);fseek(f,0,SEEK_END);long n=ftell(f);fseek(f,p,SEEK_SET);return n;}
static inline size_t BE_Cross_readInt16LEBuffer(FILE*f,void*b,size_t n){return fread(b,1,n,f);}
#define BE_Cross_Swap16LE(x) (x)
