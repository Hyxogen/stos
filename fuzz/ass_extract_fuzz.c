#include <sub_new.c>
#include <stdio.h>
#include <stdlib.h>
#include <stddef.h>
#include <assert.h>
#include <string.h>

#define BUFFER_SIZE 1024

static char *read_input(FILE *stream)
{
	char buffer[BUFFER_SIZE];
	char *result = NULL;
	size_t size = 0;
	size_t count = 0;
	size_t nread;

	while ((nread = fread(buffer, 1, BUFFER_SIZE, stream))) {
		while (count + nread >= size) {
			size_t new_size = (size + 1) * 2;
			result = realloc(result, new_size);
			assert(result != NULL);
			size = new_size;
		}
		memcpy(result + count, buffer, nread);
		count += nread;
	}
	if (count == size) {
		result = realloc(result, size + 1);
		assert(result != NULL);
	}
	result[count] = '\0';
	assert(ferror(stdin) == 0);
	return result;
}

int main(int argc, char **argv)
{
	char *input = NULL;
	if (argc == 1) {
		input = read_input(stdin);
	} else {
		FILE *file = fopen(argv[1], "r");
		assert(file != NULL);
		input = read_input(file);
		assert(fclose(file) == 0);
	}	
	char *out = NULL;
	size_t len;
	int styled;
	
	if (input == NULL)
		return 0;
	stos_ass_extract(&out, &len, &styled, input);
	free(input);
	free(out);
	return 0;
}
