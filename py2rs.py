import sys
import ast
import json


class ProtocolDefinition(object):
    def __init__(self, module):
        assigns = dict()
        for element in module.body:
            if isinstance(element, ast.Assign) and len(element.targets) == 1:
                name = element.targets[0].id
                assigns[name] = ast.literal_eval(element.value)
        self.__dict__.update(assigns)


def load_module(filename):
    with open(filename) as fh:
        return ast.parse(fh.read())

def load_protocol(filename):
    return ProtocolDefinition(load_module(filename))


def typeinfo_array_to_rs(bounds, typeid):
    yield "TypeInfo::Array {{ bounds: IntBounds {{ min: {}, bitlen: {} }}, typeid: {} }},".\
        format(bounds[0], bounds[1], typeid)


def typeinfo_bitarray_to_rs(bounds):
    yield "TypeInfo::BitArray {{ len: IntBounds {{ min: {}, bitlen: {} }} }},".\
        format(bounds[0], bounds[1])

def typeinfo_blob_to_rs(bounds):
    yield "TypeInfo::Blob {{ len: IntBounds {{ min: {}, bitlen: {} }} }},".\
        format(bounds[0], bounds[1])


def typeinfo_bool_to_rs():
    yield "TypeInfo::Bool,"


def typeinfo_choice_to_rs(bounds, fields):
    yield "TypeInfo::Choice {"
    yield "    bounds: IntBounds {{ min: {}, bitlen: {} }},".format(*bounds)
    yield "    types: phf_map! {"
    for (key, (name, typeid)) in fields.iteritems():
        yield "        {}_u32 => ({}, {}),".format(key, json.dumps(name), typeid)
    yield "    },"
    yield "},"


def typeinfo_fourcc_to_rs():
    yield "TypeInfo::FourCC,"


def typeinfo_int_to_rs(bounds):
    yield "TypeInfo::Int {{ bounds: IntBounds {{ min: {}, bitlen: {} }} }},".\
        format(bounds[0], bounds[1])


def typeinfo_null_to_rs():
    yield "TypeInfo::Null,"


def typeinfo_optional_to_rs(typeid):
    yield "TypeInfo::Optional {{ typeid: {} }},".\
        format(typeid)


def typeinfo_real32_to_rs(args):
    yield "TypeInfo::Real32,"


def typeinfo_real64_to_rs(args):
    yield "TypeInfo::Real64,"


def typeinfo_struct_to_rs(fields):
    yield "TypeInfo::Struct(Struct {"
    yield "    fields: &["
    for (name, typeid, tag) in fields:
        yield "        ({}, {}, {}),".format(json.dumps(name), typeid, tag)
    yield "    ],"
    yield "}),"


typeinfo_to_rs_map = {
    '_array': typeinfo_array_to_rs,
    '_bitarray': typeinfo_bitarray_to_rs,
    '_blob': typeinfo_blob_to_rs,
    '_bool': typeinfo_bool_to_rs,
    '_choice': typeinfo_choice_to_rs,
    '_fourcc': typeinfo_fourcc_to_rs,
    '_int': typeinfo_int_to_rs,
    '_null': typeinfo_null_to_rs,
    '_optional': typeinfo_optional_to_rs,
    '_real32': typeinfo_real32_to_rs,
    '_real64': typeinfo_real64_to_rs,
    '_struct': typeinfo_struct_to_rs,
}

def typeinfo_to_rs(typeinfo):
    return typeinfo_to_rs_map[typeinfo[0]](*typeinfo[1])


def game_event_type_to_rs(game_event_type):
    (gametypeid, (typeid, name)) = game_event_type
    yield "{}_u32 => ({}, {}),".format(gametypeid, typeid, json.dumps(name))


if __name__ == '__main__':
    protocol = load_protocol(sys.argv[1])

    print('''use super::{''')
    print('''    TypeInfo,''')
    print('''    IntBounds,''')
    print('''    Struct,''')
    print('''};''')
    print('''use phf::Map as PhfMap;''')
    print('''''')

    print('''pub static REPLAY_HEADER_TYPEID: u32 = {};'''.format(protocol.replay_header_typeid))
    print('''''')

    print('''pub static GAME_EVENTID_TYPEID: u32 = {};'''.format(protocol.game_eventid_typeid))
    print('''''')

    print('''pub static GAME_EVENT_TYPES: PhfMap<u32, (u32, &'static str)> = phf_map! {''')
    for game_event_type in sorted(protocol.game_event_types.iteritems()):
        for line in game_event_type_to_rs(game_event_type):
            print("    {}".format(line))
    print('''};''')
    print('''''')

    print('''pub static MESSAGE_EVENTID_TYPEID: u32 = {};'''.format(protocol.message_eventid_typeid))
    print('''''')

    print('''pub static MESSAGE_EVENT_TYPES: PhfMap<u32, (u32, &'static str)> = phf_map! {''')
    for game_event_type in sorted(protocol.message_event_types.iteritems()):
        for line in game_event_type_to_rs(game_event_type):
            print("    {}".format(line))
    print('''};''')
    print('''''')


    print('''pub static TYPEINFOS: &'static [TypeInfo] = &[''')
    for (idx, typeinfo) in enumerate(protocol.typeinfos):
        print("    // #{}".format(idx))
        for line in typeinfo_to_rs(typeinfo):
            print("    {}".format(line))
    print('''];''')
    print('''''')
