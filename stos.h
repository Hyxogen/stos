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
#include <libavcodec/avcodec.h>

#ifndef STOS_AVIO_BUFFER_SIZE
# define STOS_AVIO_BUFFER_SIZE 4096
#endif
#if STOS_AVIO_BUFFER_SIZE < 0
# error "STOS_AVIO_BUFFER_SIZE must be a positive integer"
#endif

enum stos_error
{
	STOS_OK = 0,
	STOS_EINVAL,
	STOS_ENOMEM,
	STOS_UNSUP,
        STOS_EIO,
        STOS_EDECODE,
        STOS_EREAD_FRAME,
        STOS_EBADF,
        STOS_ENOSUB,
        STOS_EUNKNOWN
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

struct buffer
{
        const unsigned char *ptr;
        size_t size;
};

struct ifile
{
        AVFormatContext *fctx;
        int isblob;
};

struct istream
{
        AVStream *stream;
        AVCodecContext *dec_ctx;
        const AVCodec *codec;
};

const char *stos_get_error(enum stos_error error);
void stos_destroy_rect(struct rect *rect);
void stos_destroy_sub(struct subtitle *sub);
void stos_destroy_subs(struct subtitle *sub, size_t n);
enum stos_error stos_open(struct ifile *file, const char *url);
enum stos_error stos_blob(struct ifile *file, const void *buffer, size_t size);
void stos_close(struct ifile *file);
enum stos_error stos_convert_file(struct subtitle **dst, size_t *num_subs,
				  int stream_idx, struct ifile *file);
#endif
