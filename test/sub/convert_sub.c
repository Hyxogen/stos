// sub conversion tests

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
#include <sub.c>
#include <assert.h>
#include <limits.h>
#include <string.h>
#include <stdlib.h>

/*
  Add tests for:
  calloc overflow
  invalid rectangle in the middle of correct rectangles
  invalid rectangle
*/
int main(void)
{
	{
		AVSubtitle sub;
		sub.start_display_time = 0;
		sub.end_display_time = 0;
		sub.num_rects = 0;
		sub.rects = NULL;

		struct subtitle res;
		assert(stos_convert_sub(&res, &sub) != STOS_OK);
	}
	{
		AVSubtitle sub;
		sub.start_display_time = 0;
		sub.end_display_time = 0;
		sub.num_rects = 3;
		sub.rects = calloc(sub.num_rects, sizeof(*sub.rects));
		assert(sub.rects != NULL);

		AVSubtitleRect rect0;
		rect0.type = SUBTITLE_TEXT;
		rect0.text = "Hello World!";
		sub.rects[0] = &rect0;

		AVSubtitleRect rect1;
		rect1.type = SUBTITLE_ASS;
		rect1.ass = "invalid ass";
		sub.rects[1] = &rect1;

		AVSubtitleRect rect2;
		rect2.type = SUBTITLE_ASS;
		rect2.ass = "348,0,Default,,0,0,0,,息を合わせて…";
		sub.rects[2] = &rect2;

		struct subtitle res;
		enum stos_error status = stos_convert_sub(&res, &sub);
		free(sub.rects);
		assert(status != STOS_OK);
	}
	{
		AVSubtitle sub;
		sub.start_display_time = 0;
		sub.end_display_time = 0;
		sub.num_rects = 3;
		sub.rects = calloc(sub.num_rects, sizeof(*sub.rects));
		assert(sub.rects != NULL);

		AVSubtitleRect rect0;
		rect0.type = SUBTITLE_TEXT;
		rect0.text = "Hello World!";
		sub.rects[0] = &rect0;

		AVSubtitleRect rect1;
		rect1.type = SUBTITLE_ASS;
		rect1.ass = "347,0,Default,,0,0,0,,Hello There";
		sub.rects[1] = &rect1;

		AVSubtitleRect rect2;
		rect2.type = SUBTITLE_ASS;
		rect2.ass = "348,0,Default,,0,0,0,,息を合わせて…";
		sub.rects[2] = &rect2;

		struct subtitle res;
		assert(stos_convert_sub(&res, &sub) == STOS_OK);
		assert(res.num_rects == 3);

		assert(res.rects[0].type == STOS_TYPE_TEXT);
		assert(strcmp(res.rects[0].text, "Hello World!") == 0);

		assert(res.rects[1].type == STOS_TYPE_TEXT);
		assert(strcmp(res.rects[1].text, "Hello There") == 0);

		assert(res.rects[2].type == STOS_TYPE_TEXT);
		assert(strcmp(res.rects[2].text, "息を合わせて…") == 0);

		stos_destroy_sub(&res);
		free(sub.rects);
	}
	return 0;
}
