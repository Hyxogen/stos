#include <stos.h>
#include <ctype.h>
#include <string.h>
#include <stdlib.h>
#include <libavcodec/avcodec.h>

/* custom implementation of strdup */
char* stos_strdup(const char *str)
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

/* perform strchr(strchr(...) + 1, ch) n times */
static char *stos_nstrchr(const char *str, int ch, size_t n)
{
	while (n > 0 && (str = strchr(str, ch)) != NULL) {
		n -= 1;
		str += (n != 0);
	}
	return (char *)str;
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
		return STOS_EINVAL;
	}
}

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
