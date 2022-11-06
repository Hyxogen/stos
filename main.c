// main entrypoint stos

// Copyright (C) 2022 Daan Meijer
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
#include <stdio.h>
#include <errno.h>
#include <string.h>
#include <stos.h>
#include <stdlib.h>

int print_subs(struct ifile *file, int stream_idx)
{
        struct subtitle *subs = NULL;
	size_t n = 0;
	int status = 0;
        enum stos_error error;

	error = stos_convert_file(&subs, &n, stream_idx, file);
	if (error != STOS_OK) {
		fprintf(stderr, "%s\n", stos_get_error(error));
		status = -1;
		goto cleanup;
	}
	
	for (size_t sub_idx = 0; sub_idx < n; ++sub_idx) {
		const struct subtitle *sub = &subs[sub_idx];
		for (size_t rect_idx = 0; rect_idx < sub->num_rects;
		     ++rect_idx) {
			fprintf(stdout, "%u-%u: %s\n", sub->start_time,
				sub->end_time, sub->rects[rect_idx].text);
		}
	}

 cleanup:
	if (subs != NULL) {
                stos_destroy_subs(subs, n);
                free(subs);
        }
	return status;
}

int main(int argc, char **argv) 
{
	if (argc != 2) {
		fprintf(stderr, "usage: %s <in_file>\n", argv[0]);
		return EXIT_FAILURE;
	}
        struct ifile file;
	enum stos_error error = stos_open(&file, argv[1]);
	if (error != STOS_OK) {
		fprintf(stderr, "%s: %s\n", argv[1], stos_get_error(error));
		return EXIT_FAILURE;
	}

        int status = EXIT_SUCCESS;
        if (print_subs(&file, -1) < 0)
                status = EXIT_FAILURE;
        stos_close(&file);
        return status;
}
