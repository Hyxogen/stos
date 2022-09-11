// stos public interface

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
#ifndef STOS_H
#define STOS_H

#include <stddef.h>
#include <libavformat/avformat.h>

struct file_info 
{
	AVFormatContext *fctx;
};

struct subtitle
{
	unsigned int start_time;
	unsigned int end_time;
	char **text;
};

int get_file_info(struct file_info *info, const char *url);
void del_file_info(struct file_info *info);

struct subtitle *get_subs(const struct file_info *info, int stream_idx, size_t *n);
void del_sub(struct subtitle *sub);

const char* stos_get_error(void);

#endif
