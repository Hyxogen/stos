TARGET		:= stos

CC		:= cc

SOURCE_FILES	:= main.c sub.c
OBJECT_FILES	:= $(SOURCE_FILES:.c=.o)
DEPEND_FILES	:= $(SOURCE_FILES:.c=.d)

CFLAGS		:= -Wall -Wextra -std=c99 -pedantic -MMD -MP \
		   -I.
LFLAGS		:= -Wall -Wextra -std=c99 -pedantic -lavcodec -lavutil \
		   -lavformat

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
