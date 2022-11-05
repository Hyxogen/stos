TARGET		:= stos

CC		:= cc

SOURCE_FILES	:= main.c sub_new.c
OBJECT_FILES	:= $(SOURCE_FILES:.c=.o)
DEPEND_FILES	:= $(SOURCE_FILES:.c=.d)

CFLAGS		:= -Wall -Wextra -std=c99 -pedantic -MMD -MP -g3 -O0 \
		   -I. -fsanitize=address,undefined
LFLAGS		:= -Wall -Wextra -std=c99 -pedantic -g3 -O0 -lavcodec -lavutil \
		   -lavformat -fsanitize=address,undefined

all: $(TARGET)

$(TARGET): $(OBJECT_FILES)
	$(CC) -o $(TARGET) $(OBJECT_FILES) $(LFLAGS)

-include $(DEPEND_FILES)
%.o: %.c Makefile
	$(CC) $(CFLAGS) -c $< -o $@

re:
	${MAKE} clean
	${MAKE}

clean:
	rm -f $(OBJECT_FILES)
	rm -f $(DEPEND_FILES)
	rm -f $(TARGET)
