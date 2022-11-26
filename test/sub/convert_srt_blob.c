// srt blob test

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
#include <string.h>

int main() {
        struct ifile file;
        const char *sub = "1\n00:00:00,000 --> 00:00:01,000\nこんにちは";
        assert(stos_blob(&file, sub, strlen(sub)) == STOS_OK);

        struct subtitle_list list;
        assert(stos_convert_file(&list, -1, &file) == STOS_OK);
        assert(list.count == 1);
        assert(list.subs[0].num_rects == 1);
        assert(strcmp(list.subs[0].rects[0].text, "こんにちは") == 0);
	stos_destroy_sub_list(&list);
        stos_close(&file);
        return 0;
}
