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

enum stos_error
{
	STOS_OK = 0,
	STOS_EINVAL,
	STOS_ENOMEM,
	STOS_UNSUP
};

enum rect_type
{
	STOS_TYPE_TEXT,
	STOS_TYPE_BITMAP
};

struct rect 
{
	enum rect_type type;
	char *text;
};

struct subtitle
{
	unsigned int start_time;
	unsigned int end_time;
	
	size_t num_rects;
	struct rect *rects;
};

void stos_destroy_rect(struct rect *rect);
void stos_destroy_sub(struct subtitle *sub);

#endif
