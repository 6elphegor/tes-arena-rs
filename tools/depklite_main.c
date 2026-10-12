#include "depklite.h"
#include <stdlib.h>
int main(int c,char**v){FILE*f=fopen(v[1],"rb");int n=1<<20;unsigned char*b=calloc(n,1);uint16_t x;
if(!depklite_unpack(f,b,n,&x))puts("warn");FILE*o=fopen(v[2],"wb");fwrite(b,1,n,o);fclose(o);printf("ok extra=%u\n",x);}
