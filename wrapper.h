#include "obs/libobs/obs-module.h"

// there doesn't seem to be any official way of getting info.type_data during creation :(
typedef void *pthread_mutex_t;

struct obs_context_data
{
    char *name;
    void *data;
    obs_data_t *settings;
    signal_handler_t *signals;
    proc_handler_t *procs;
    enum obs_obj_type type;

    DARRAY(obs_hotkey_id)
    hotkeys;
    DARRAY(obs_hotkey_pair_id)
    hotkey_pairs;
    obs_data_t *hotkey_data;

    DARRAY(char *)
    rename_cache;
    void *rename_cache_mutex;

    void *mutex;
    struct obs_context_data *next;
    struct obs_context_data **prev_next;

    bool private;
};

struct obs_source
{
    struct obs_context_data context;
    struct obs_source_info info;

    // remainder omitted
};