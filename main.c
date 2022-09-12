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
// along with this program.  If not, see
// <https://www.gnu.org/licenses/>.
#include <stdio.h>
#include <errno.h>
#include <string.h>
#include "libavcodec/avcodec.h"
#include "libavformat/avformat.h"
#include <stos.h>
#include <stdlib.h>

//todo fix extracting subtitles out of video files
//	maybe this can be fixed by using a parser?
int main(int argc, char **argv) 
{
	struct file_info info;
	
	if (argc != 2) {
		fprintf(stderr, "usage: %s <in_file>\n", argv[0]);
		return EXIT_FAILURE;
	}
	if (get_file_info(&info, argv[1]) < 0) {
		fprintf(stderr, "%s: %s\n", argv[1], stos_get_error());
		return EXIT_FAILURE;
	}
	
	struct subtitle *subs;
	size_t n = 0;
	
	if ((subs = get_subs(&info, -1, &n)) == NULL) {
		fprintf(stderr, "%s: %s\n", argv[1], stos_get_error());
		return EXIT_FAILURE;
	}
	
	for (size_t sub_idx = 0; sub_idx < n; ++sub_idx) {
		const struct subtitle *sub = &subs[sub_idx];
		for (size_t txt_idx = 0; txt_idx < sub->num_text; ++txt_idx) {
			fprintf(stdout, "%u-%u: %s\n", sub->start_time,
				sub->end_time, sub->text[txt_idx]);
		}
	}
	
	del_subs(subs, n);
	del_file_info(&info);
	return EXIT_SUCCESS;
}
