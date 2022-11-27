#include <stos.h>
#include <ctype.h>
#include <string.h>
#include <stdlib.h>
#include <libavcodec/avcodec.h>

/* custom implementation of strdup */
char *stos_strdup(const char *str)
{
	size_t len = strlen(str);
	char *result = malloc(len + 1);

	if (result != NULL) {
		memcpy(result, str, len);
		result[len] = '\0';
	}
	return result;
}

/* free resources of a rect */
void stos_destroy_rect(struct rect *rect)
{
	free(rect->text);
}

/* free resources of a subtitle */
void stos_destroy_sub(struct subtitle *sub)
{
	for (size_t i = 0; i < sub->num_rects; ++i) {
		stos_destroy_rect(&sub->rects[i]);
	}
	free(sub->rects);
}

/* free resources of a subtitle list */
void stos_destroy_sub_list(struct subtitle_list *list)
{
	for (size_t i = 0; i < list->count; ++i) {
		stos_destroy_sub(list->subs + i);
	}
	free(list->subs);
}

/* initialize subtitle list with default values */
void stos_init_sub_list(struct subtitle_list *list)
{
	list->subs = NULL;
	list->count = 0;
	list->size = 0;
}

/* perform strchr(strchr(...) + 1, ch) n times */
static char *stos_nstrchr(const char *str, int ch, size_t n)
{
	while (n > 0 && (str = strchr(str, ch)) != NULL) {
		n -= 1;
		str += (n != 0);
	}
	return (char *)str;
}

/* retrieve human readable string of an error */
const char *stos_get_error(enum stos_error error)
{
	switch (error) {
	case STOS_OK:
		return "no error";
	case STOS_EINVAL:
		return "an invalid argument was passed";
	case STOS_ENOMEM:
		return "the process ran out of memory";
	case STOS_UNSUP:
		return "format is not supported";
	case STOS_EIO:
		return "could not properly read from file";
	case STOS_ENOSUB:
		return "could not retrieve subtitle stream";
	case STOS_EREAD_FRAME:
		return "could not read next frame of a stream";
	case STOS_EDECODE:
		return "could not decode a packet from a stream";
	case STOS_EBADF:
		return "could not open the file";
	case STOS_EUNKNOWN:
	default:
		return "an unknown error ocurred, please report this";
	}
}

/* extract the text part out of an ass event without the style */
static enum stos_error stos_ass_extract(char **out, size_t *len, int *styled,
					const char *event)
{
	event = stos_nstrchr(event, ',', (size_t)8);
	if (event == NULL)
		return STOS_EINVAL;

	event += 1;
	size_t event_len = strlen(event);
	char *text = malloc(event_len + 1);
	if (text == NULL)
		return STOS_ENOMEM;

	size_t i = 0;
	size_t j = 0;
	size_t brackets = 0;
	int has_style = 0;
	while (i < event_len) {
		if (event[i] == '{') {
			has_style = 1;
			brackets += 1;
		} else if (event[i] == '}' && brackets != 0) {
			brackets -= 1;
		} else if (event[i] == '\\' && i + 1 < event_len &&
			   tolower(event[i] == 'n')) {
			text[j] = '\n';
			j += 1;
		} else if (brackets == 0) {
			text[j] = event[i];
			j += 1;
		}
		i += 1;
	}
	text[j] = '\0';
	if (out != NULL)
		*out = text;
	else
		free(text);
	if (len != NULL)
		*len = j;
	if (styled != NULL)
		*styled = has_style;
	return STOS_OK;
}

static enum stos_error stos_convert_bitm_rect(struct rect *dst,
					      const AVSubtitleRect *rect)
{
	/* TODO: implement */
	(void)dst;
	(void)rect;
	return STOS_UNSUP;
}

static enum stos_error stos_convert_text_rect(struct rect *dst,
					      const AVSubtitleRect *rect)
{
	dst->type = STOS_TYPE_TEXT;
	dst->text = stos_strdup(rect->text);
	if (dst->text == NULL)
		return STOS_ENOMEM;
	return STOS_OK;
}

static enum stos_error stos_convert_ass_rect(struct rect *dst,
					     const AVSubtitleRect *rect)
{
	dst->type = STOS_TYPE_TEXT;
	return stos_ass_extract(&dst->text, NULL, NULL, rect->ass);
}

static enum stos_error stos_convert_rect(struct rect *dst,
					 const AVSubtitleRect *rect)
{
	switch (rect->type) {
	case SUBTITLE_BITMAP:
		return stos_convert_bitm_rect(dst, rect);
	case SUBTITLE_TEXT:
		return stos_convert_text_rect(dst, rect);
	case SUBTITLE_ASS:
		return stos_convert_ass_rect(dst, rect);
	case SUBTITLE_NONE:
	default:
		return STOS_UNSUP;
	}
}

/* convert an AVSubtitle to a struct subtitle */
static enum stos_error stos_convert_sub(struct subtitle *dst,
					const AVSubtitle *sub)
{
	dst->start_time = sub->start_display_time;
	dst->end_time = sub->end_display_time;

	if (sub->num_rects == 0)
		return STOS_EINVAL;

	dst->num_rects = 0;
	dst->rects = calloc(sub->num_rects, sizeof(*dst->rects));
	if (dst->rects == NULL)
		return STOS_ENOMEM;

	enum stos_error status = STOS_OK;
	for (size_t i = 0; i < sub->num_rects; ++i) {
		status = stos_convert_rect(&dst->rects[i], sub->rects[i]);
		if (status != STOS_OK)
			goto error;
		dst->num_rects += 1;
	}
	return STOS_OK;
error:
	stos_destroy_sub(dst);
	return status;
}

/* insert possibly missing subtitle start and end times */
void stos_subtitle_fix_timings(AVSubtitle *sub, const AVPacket *pkt)
{
	if (pkt->dts != AV_NOPTS_VALUE) {
		sub->start_display_time = (uint32_t)pkt->dts;
		sub->end_display_time = (uint32_t)pkt->pts;
	}
}

/* convert a packet from a subtitle stream to a struct subtitle */
static enum stos_error stos_convert_packet(struct subtitle *dst, AVPacket *pkt,
					   struct istream *stream)
{
	AVSubtitle avsub;
	int got = 0;

	if (avcodec_decode_subtitle2(stream->dec_ctx, &avsub, &got, pkt) < 0 ||
	    got == 0)
		return STOS_EDECODE;
	stos_subtitle_fix_timings(&avsub, pkt);
	enum stos_error status = stos_convert_sub(dst, &avsub);
	avsubtitle_free(&avsub);
	return status;
}

/* convert a packet from a subtitle stream and add it to the subtitle list */
static enum stos_error
stos_process_subtitle(void *opaque, struct istream *stream, AVPacket *pkt)
{
	struct subtitle_list *list = opaque;

	if (list->count == list->size) {
		size_t new_size = list->size == 0 ? 1 : list->size * 2;
		struct subtitle *new_subs =
			realloc(list->subs, new_size * sizeof(*list->subs));
		if (new_subs == NULL)
			return STOS_ENOMEM;
		list->size = new_size;
		list->subs = new_subs;
	}

	enum stos_error status =
		stos_convert_packet(&list->subs[list->count], pkt, stream);
	list->count += 1;
	return status;
}

/* call proc for each packet of istream */
enum stos_error stos_process_stream(
	struct istream *istream, struct ifile *file,
	enum stos_error (*proc)(void *, struct istream *stream, AVPacket *),
	void *opaque)
{
	AVPacket *pkt = av_packet_alloc();
	if (pkt == NULL)
		return STOS_ENOMEM;

	int rc;
	enum stos_error status = STOS_OK;
	while ((rc = av_read_frame(file->fctx, pkt)) >= 0) {
		if (pkt->stream_index != istream->stream->index)
			goto loop;

		status = proc(opaque, istream, pkt);
loop:
		av_packet_unref(pkt);
		if (status != STOS_OK)
			break;
	}
	av_packet_free(&pkt);
	if (rc < 0 && rc != AVERROR_EOF)
		status = STOS_EREAD_FRAME;
	return status;
}

/* find the first stream that matches a predicate */
/* returns -1 on no stream found matching the predicate */
int stos_find_istream(const struct ifile *file,
		      int (*predicate)(const AVStream *))
{
	const int max = file->fctx->nb_streams >= INT_MAX ?
				INT_MAX :
				(int)file->fctx->nb_streams;
	for (int idx = 0; idx < max; ++idx) {
		if (predicate(file->fctx->streams[idx]))
			return idx;
	}
	return -1;
}

/* get a initialized struct istream */
static enum stos_error
stos_get_istream(struct istream *dst, const struct ifile *file, int stream_idx)
{
	if (stream_idx < 0 || (unsigned int)stream_idx > file->fctx->nb_streams)
		return STOS_ENOSUB;

	AVStream *stream = file->fctx->streams[(unsigned int)stream_idx];
	const AVCodec *codec = avcodec_find_decoder(stream->codecpar->codec_id);
	if (codec == NULL)
		return STOS_UNSUP;

	AVCodecContext *dec_ctx = avcodec_alloc_context3(codec);
	if (dec_ctx == NULL)
		return STOS_ENOMEM;
	dec_ctx->thread_count = 12;
	dec_ctx->thread_type = FF_THREAD_FRAME;

	enum stos_error status = STOS_EUNKNOWN;
	AVDictionary *opts = NULL;
	if (av_dict_set(&opts, "", "", 0) < 0)
		goto cleanup_decoder;
	if (avcodec_open2(dec_ctx, codec, &opts))
		goto cleanup_decoder;

	status = STOS_OK;

	struct istream istream = { .stream = stream,
				   .codec = codec,
				   .dec_ctx = dec_ctx };

	if (dst != NULL) {
		*dst = istream;
		goto cleanup_dictionary;
	}
cleanup_decoder:
	if (dec_ctx != NULL)
		avcodec_free_context(&dec_ctx);
cleanup_dictionary:
	if (opts != NULL)
		av_dict_free(&opts);
	return status;
}

void stos_destroy_istream(struct istream *istream)
{
	avcodec_free_context(&istream->dec_ctx);
}

static int stos_is_sub(const AVStream *stream)
{
	enum AVCodecID id = stream->codecpar->codec_id;
	return id == AV_CODEC_ID_DVD_SUBTITLE ||
	       id == AV_CODEC_ID_DVB_SUBTITLE || id == AV_CODEC_ID_TEXT ||
	       id == AV_CODEC_ID_XSUB || id == AV_CODEC_ID_SSA ||
	       id == AV_CODEC_ID_MOV_TEXT ||
	       id == AV_CODEC_ID_HDMV_PGS_SUBTITLE ||
	       id == AV_CODEC_ID_DVB_TELETEXT || id == AV_CODEC_ID_SRT ||
	       id == AV_CODEC_ID_MICRODVD || id == AV_CODEC_ID_EIA_608 ||
	       id == AV_CODEC_ID_JACOSUB || id == AV_CODEC_ID_SAMI ||
	       id == AV_CODEC_ID_REALTEXT || id == AV_CODEC_ID_STL ||
	       id == AV_CODEC_ID_SUBVIEWER1 || id == AV_CODEC_ID_SUBVIEWER ||
	       id == AV_CODEC_ID_SUBRIP || id == AV_CODEC_ID_WEBVTT ||
	       id == AV_CODEC_ID_MPL2 || id == AV_CODEC_ID_VPLAYER ||
	       id == AV_CODEC_ID_PJS || id == AV_CODEC_ID_ASS ||
	       id == AV_CODEC_ID_HDMV_TEXT_SUBTITLE || id == AV_CODEC_ID_TTML ||
	       id == AV_CODEC_ID_ARIB_CAPTION;
}

enum stos_error stos_open(struct ifile *file, const char *url)
{
	/* TODO check if path is a dir */
	file->isblob = 0;
	file->fctx = NULL;
	if (avformat_open_input(&file->fctx, url, NULL, NULL) < 0)
		return STOS_EBADF;
	if (avformat_find_stream_info(file->fctx, NULL) < 0) {
		avformat_close_input(&file->fctx);
		return STOS_EIO;
	}
	return STOS_OK;
}

static int stos_read_packet(void *opaque, unsigned char *buf, int buf_ssize)
{
	struct buffer *data = (struct buffer *)opaque;
	unsigned int buf_size = (unsigned int)buf_ssize;
	if (data->size < buf_size)
		buf_size = (unsigned int)data->size;

	if (buf_size == 0)
		return AVERROR_EOF;
	memcpy(buf, data->ptr, buf_size);
	data->ptr += buf_size;
	data->size -= buf_size;
	return (int)buf_size;
}

enum stos_error stos_blob(struct ifile *file, const void *buffer, size_t size)
{
	file->isblob = 1;
	file->fctx = avformat_alloc_context();
	if (file->fctx == NULL)
		return STOS_ENOMEM;

	struct buffer data = {
		.ptr = buffer,
		.size = size,
	};

	enum stos_error status = STOS_OK;

	unsigned char *avio_buffer = av_malloc(STOS_AVIO_BUFFER_SIZE);
	AVIOContext *avio_ctx = NULL;
	if (avio_buffer == NULL) {
		status = STOS_ENOMEM;
		goto error;
	}

	avio_ctx = avio_alloc_context(avio_buffer, STOS_AVIO_BUFFER_SIZE, 0,
				      &data, &stos_read_packet, NULL, NULL);
	if (avio_ctx == NULL) {
		status = STOS_ENOMEM;
		goto error;
	}

	file->fctx->pb = avio_ctx;

	if (avformat_open_input(&file->fctx, NULL, NULL, NULL) < 0) {
		status = STOS_EINVAL;
		goto error;
	}

	if (avformat_find_stream_info(file->fctx, NULL) >= 0)
		return STOS_OK;

	avformat_close_input(&file->fctx);
	status = STOS_EINVAL;
error:
	if (file->fctx != NULL)
		avformat_free_context(file->fctx);
	if (avio_ctx != NULL)
		av_freep(&avio_ctx->buffer);
	avio_context_free(&avio_ctx);
	return status;
}

void stos_close(struct ifile *file)
{
	if (file->isblob) {
		if (file->fctx->pb->buffer != NULL)
			av_freep(&file->fctx->pb->buffer);
		avio_context_free(&file->fctx->pb);
	}
	avformat_close_input(&file->fctx);
}

/* convert subtitle stream stream_idx to struct subtitle array */
/* this function will select the first subtitle stream if stream_idx < 0 */
enum stos_error stos_convert_file(struct subtitle_list *dst, int stream_idx,
				  struct ifile *file)
{
	if (stream_idx < 0)
		stream_idx = stos_find_istream(file, stos_is_sub);

	struct istream stream;
	struct subtitle_list list;
	enum stos_error status = STOS_OK;

	stos_init_sub_list(&list);
	status = stos_get_istream(&stream, file, stream_idx);
	if (status == STOS_OK) {
		status = stos_process_stream(&stream, file,
					     stos_process_subtitle, &list);
		stos_destroy_istream(&stream);
	}
	if (dst != NULL)
		*dst = list;
	else
		stos_destroy_sub_list(&list);
	return status;
}
