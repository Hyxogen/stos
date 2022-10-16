#include <stos.h>
#include <ctype.h>
#include <string.h>
#include <stdlib.h>

/* perform strchr(strchr(...) + 1, ch) n times */
static char *stos_nstrchr(const char *str, int ch, size_t n)
{
	while (n > 0 && (str = strchr(str, ch)) != NULL) {
		n -= 1;
		str += 1;
	}
	return (char *) str;
}

/* extract the text part out of an ass event without the style */
static enum stos_error stos_ass_extract(char **out, size_t *len, int *styled,
					const char *event)
{
	event = stos_nstrchr(event, ',', (size_t) 8);
	if (event == NULL)
		return STOS_EVAL;

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
		i += 1; }
	text[j] = '\0';
	if (out != NULL)
		*out = text;
	if (len != NULL)
		*len = j;
	if (styled != NULL)
		*styled = has_style;
	return STOS_SUCCESS;
}
