// simple text conversion tests

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
#include <sub_new.c>
#include <assert.h>
#include <stddef.h>
#include <string.h>

int main(void)
{
	{
		AVSubtitleRect rect;
		rect.type = SUBTITLE_NONE;

		struct rect res;
		assert(stos_convert_rect(&res, &rect) != STOS_OK);
	}
	{
		AVSubtitleRect rect;
		rect.type = SUBTITLE_TEXT;
		rect.text = "Hello World!";

		struct rect res;
		assert(stos_convert_rect(&res, &rect) == STOS_OK);
		assert(res.type == STOS_TYPE_TEXT);
		assert(strcmp(rect.text, (char *)res.text) == 0);
		stos_destroy_rect(&res);
	}
	{
		AVSubtitleRect rect;
		rect.type = SUBTITLE_ASS;
		rect.ass = "348,0,Default,,0,0,0,,息を合わせて…";
		rect.text = "invalid ass";

		struct rect res;
		assert(stos_convert_rect(&res, &rect) == STOS_OK);
		assert(res.type == STOS_TYPE_TEXT);
		assert(strcmp("息を合わせて…", (char *)res.text) == 0);
		stos_destroy_rect(&res);
	}
	{
		AVSubtitleRect rect;
		rect.type = SUBTITLE_ASS;
		rect.ass = "invalid ass";
		rect.text = "348,0,Default,,0,0,0,,息を合わせて…";
		
		struct rect res;
		assert(stos_convert_rect(&res, &rect) != STOS_OK);
	}
	return 0;
}
