#include <sub_new.c>
#include <stdio.h>
#include <stdlib.h>
#include <stddef.h>
#include <assert.h>
#include <string.h>

__AFL_FUZZ_INIT();
int main(void)
{
#ifdef __AFL_HAVE_MANUAL_CONTROL
        __AFL_INIT();
#endif
        unsigned char *buf = __AFL_FUZZ_TESTCASE_BUF;

        while (__AFL_LOOP(10000)) {
                int ilen = __AFL_FUZZ_TESTCASE_LEN;
                if (ilen <= 1)
                        continue;
                if (buf == NULL)
                        continue;
                char *out = NULL;
                size_t len;
                int styled;

                stos_ass_extract(&out, &len, &styled, buf);
                free(out);
        }
        return 0;
}
