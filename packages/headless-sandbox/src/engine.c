/* Narrow, source-only embedding. Never initialize quickjs-libc or a module loader. */
#include <windows.h>
#include <string.h>
#include "quickjs.h"

typedef struct { ULONGLONG end; int interrupted; } deadline;

unsigned long long flowtools_p22_mitigation_policy(void) {
    return PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_ALWAYS_ON |
           PROCESS_CREATION_MITIGATION_POLICY_WIN32K_SYSTEM_CALL_DISABLE_ALWAYS_ON;
}

static int interrupt(JSRuntime *runtime, void *opaque) {
    deadline *limit = opaque;
    (void)runtime;
    if (GetTickCount64() >= limit->end) {
        limit->interrupted = 1;
        return 1;
    }
    return 0;
}

/* 0 = JSON result; stable failures only, never plugin exception text. */
int flowtools_p22_evaluate(const char *source, size_t source_len,
                          char *output, size_t output_limit, size_t *output_len) {
    JSRuntime *runtime = JS_NewRuntime();
    if (!runtime) return 2;
    JS_SetMemoryLimit(runtime, 16 * 1024 * 1024);
    JS_SetMaxStackSize(runtime, 256 * 1024);
    deadline limit = { GetTickCount64() + 250, 0 };
    JS_SetInterruptHandler(runtime, interrupt, &limit);
    JSContext *context = JS_NewContext(runtime);
    if (!context) { JS_FreeRuntime(runtime); return 2; }
    JSValue value = JS_Eval(context, source, source_len, "<signed-source-fixture>",
                            JS_EVAL_TYPE_GLOBAL | JS_EVAL_FLAG_STRICT);
    int status = 0;
    if (JS_IsException(value)) {
        status = limit.interrupted ? 3 : 2;
    } else {
        JSValue json = JS_JSONStringify(context, value, JS_UNDEFINED, JS_UNDEFINED);
        if (JS_IsException(json) || JS_IsUndefined(json)) {
            status = limit.interrupted ? 3 : 2;
        } else {
            size_t size = 0;
            const char *text = JS_ToCStringLen(context, &size, json);
            if (!text) status = limit.interrupted ? 3 : 2;
            else {
                if (size > output_limit) status = 4;
                else { memcpy(output, text, size); *output_len = size; }
                JS_FreeCString(context, text);
            }
        }
        JS_FreeValue(context, json);
    }
    JS_FreeValue(context, value);
    JS_FreeContext(context);
    JS_FreeRuntime(runtime);
    return status;
}
