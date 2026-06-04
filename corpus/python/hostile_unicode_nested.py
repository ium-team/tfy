def outer_unicode(name):
    message = "hé 😀 : keep spaces"
    # keep this comment
    def inner(value):
        return message + str(value)
    return inner(name)
