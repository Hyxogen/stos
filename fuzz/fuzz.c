#include "../sub.c"
#include <stddef.h>
#include <stdint.h>

int LLVMFuzzerTestOneInput(const uint8_t *Data, size_t Size) {
        struct ifile file;
        av_log_set_level(AV_LOG_QUIET);
        enum stos_error error = stos_blob(&file, Data, Size);
        if (error != STOS_OK)
                return 0;
        struct subtitle *subs = NULL;
        size_t n = 0;
        stos_convert_file(&subs, &n, -1, &file);
        if (subs != NULL) {
                stos_destroy_subs(subs, n);
                free(subs);
        }
        stos_close(&file);
        return 0;
}
